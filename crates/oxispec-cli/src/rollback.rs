//! `oxispec rollback`: put nodes back to how they were at an earlier
//! revision, in the working tree, after checking that nothing breaks.
//!
//! Nothing is committed. You review the result and commit it, which is a new
//! revision, so history is never rewritten.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Component, Path, PathBuf};

use anyhow::{Context, Result, anyhow, bail};
use oxispec_core::rollback::{self, Action};
use oxispec_core::{OUTPUT_SCHEMA_VERSION, config, index};
use serde_json::json;

use crate::git;

pub struct Options<'a> {
    pub revision: &'a str,
    pub ids: Vec<String>,
    pub all: bool,
    pub force: bool,
    pub dry_run: bool,
    pub json: bool,
    pub cache: bool,
}

/// Paths from git are relative and have no `..`, but a file is only ever
/// written or deleted when this says so.
fn is_safe(path: &Path) -> bool {
    path.components().all(|c| matches!(c, Component::Normal(_)))
}

pub fn rollback_command(root: &Path, options: &Options<'_>) -> Result<()> {
    config::load(root)?;
    if options.all && !options.ids.is_empty() {
        bail!("give node IDs or --all, not both");
    }
    if !options.all && options.ids.is_empty() {
        bail!("name the nodes to roll back, or use --all to restore everything that differs");
    }
    git::require_repository(root, "rollback")?;
    let revision = git::resolve_rev(root, options.revision)?;
    let target = git::graph_at(root, &revision)?;
    let current = index::load(root, options.cache).0;

    let ids = if options.all {
        rollback::differing_ids(&current, &target)
    } else {
        options.ids.clone()
    };
    let actions = rollback::plan(&current, &target, &ids).map_err(|e| anyhow!(e))?;
    let acting: Vec<&Action> = actions.iter().filter(|a| a.changes_the_tree()).collect();

    // Files that will be deleted, and files that will be written.
    let mut removals: Vec<PathBuf> = Vec::new();
    let mut writes: Vec<PathBuf> = Vec::new();
    for action in &acting {
        match action {
            Action::Restore { from, to, .. } => {
                if from != to {
                    removals.push(from.clone());
                }
                writes.push(to.clone());
            }
            Action::Recreate { path, .. } => writes.push(path.clone()),
            Action::Remove { path, .. } => removals.push(path.clone()),
            Action::Unchanged { .. } => {}
        }
    }
    if let Some(bad) = removals.iter().chain(&writes).find(|p| !is_safe(p)) {
        bail!("refusing to touch {}", bad.display());
    }

    // Problems that stop the rollback unless --force is given.
    let after = rollback::would_be(&current, &target, &actions);
    let new_errors = rollback::new_errors(&current, &after);
    let at_risk: Vec<PathBuf> = removals.iter().chain(&writes).cloned().collect();
    let uncommitted = git::dirty_paths(root, &at_risk)?;
    let removed: BTreeSet<&PathBuf> = removals.iter().collect();
    let occupied: Vec<&PathBuf> = acting
        .iter()
        .filter_map(|a| match a {
            // The node's own file is overwritten on purpose.
            Action::Restore { from, to, .. } if from == to => None,
            Action::Restore { to, .. } => Some(to),
            Action::Recreate { path, .. } => Some(path),
            _ => None,
        })
        .filter(|path| root.join(path).exists() && !removed.contains(path))
        .collect();
    if let Some(path) = occupied.first() {
        bail!(
            "{} already exists and is not part of this rollback, so it would be overwritten",
            path.display()
        );
    }

    if options.json {
        let out = json!({
            "schema_version": OUTPUT_SCHEMA_VERSION,
            "revision": {"commit": revision.oid, "short": revision.short, "subject": revision.subject},
            "dry_run": options.dry_run,
            "actions": actions,
            "new_errors": new_errors,
            "uncommitted": uncommitted,
        });
        println!("{}", serde_json::to_string_pretty(&out)?);
    } else {
        println!("rolling back to {revision}");
        for action in &actions {
            println!("  {}", action.describe());
        }
        if acting.is_empty() {
            println!("  nothing to do");
        }
    }

    if acting.is_empty() {
        return Ok(());
    }

    let mut blocked = Vec::new();
    if !new_errors.is_empty() {
        blocked.push(format!(
            "the rollback would introduce {} new error(s)",
            new_errors.len()
        ));
        if !options.json {
            for f in &new_errors {
                println!("  would break: {}", crate::finding_line(f));
            }
        }
    }
    if !uncommitted.is_empty() {
        blocked.push(format!(
            "{} file(s) have changes that are not committed and would be lost: {}",
            uncommitted.len(),
            uncommitted.join(", ")
        ));
    }
    if !blocked.is_empty() && !options.force {
        bail!("{}; fix that, or use --force", blocked.join(", and "));
    }
    if options.dry_run {
        if !options.json {
            println!("dry run: nothing was changed");
        }
        return Ok(());
    }

    // Read everything from git first, so a failure leaves the tree untouched.
    let mut contents: Vec<(PathBuf, Vec<u8>)> = Vec::new();
    for path in &writes {
        let bytes = git::file_at(root, &revision.oid, path)?
            .with_context(|| format!("{} is not in {}", path.display(), revision.short))?;
        contents.push((path.clone(), bytes));
    }
    for path in &removals {
        let full = root.join(path);
        if full.exists() {
            fs::remove_file(&full).with_context(|| format!("cannot delete {}", path.display()))?;
        }
    }
    for (path, bytes) in &contents {
        let full = root.join(path);
        if let Some(dir) = full.parent() {
            fs::create_dir_all(dir)?;
        }
        fs::write(&full, bytes).with_context(|| format!("cannot write {}", path.display()))?;
    }
    if !options.json {
        println!(
            "wrote {} file(s), deleted {}. Review it with `oxispec diff HEAD`, then commit. The commit is the new revision, and history is not rewritten.",
            contents.len(),
            removals.len()
        );
    }
    Ok(())
}
