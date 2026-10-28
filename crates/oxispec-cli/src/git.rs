//! Asking git which spec files changed. Used by `oxispec check --changed`.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use anyhow::{Context, Result, bail};
use oxispec_core::parse::parse_file_text;

/// The markdown files that differ from a base, relative to the project root.
#[derive(Debug, Default)]
pub struct Changes {
    pub paths: BTreeSet<PathBuf>,
    /// IDs the changed files had at the base. A deleted or renamed node is
    /// only known by its old ID.
    pub old_ids: BTreeSet<String>,
}

fn git(root: &Path, args: &[&str]) -> Result<Output> {
    Command::new("git")
        .arg("-C")
        .arg(root)
        .args(args)
        .output()
        .context("cannot run git; is it installed and on PATH?")
}

fn git_stdout(root: &Path, args: &[&str]) -> Result<Vec<u8>> {
    let out = git(root, args)?;
    if !out.status.success() {
        bail!(
            "git {} failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&out.stderr).trim()
        );
    }
    Ok(out.stdout)
}

/// Splits git's NUL separated output into names.
fn names(bytes: &[u8]) -> impl Iterator<Item = String> + '_ {
    bytes
        .split(|b| *b == 0)
        .filter(|s| !s.is_empty())
        .map(|s| String::from_utf8_lossy(s).into_owned())
}

/// Markdown files that differ from `base` in the working tree (staged,
/// unstaged and untracked), and the IDs those files had at `base`.
///
/// In a repository with no commits yet, `HEAD` does not exist, so every
/// tracked or untracked file counts as changed.
pub fn changes(root: &Path, base: &str) -> Result<Changes> {
    let inside = git(root, &["rev-parse", "--is-inside-work-tree"])?;
    if !inside.status.success() {
        bail!(
            "`check --changed` needs a git repository, and {} is not inside one",
            root.display()
        );
    }
    let revision = format!("{base}^{{commit}}");
    let base_exists = git(root, &["rev-parse", "--verify", "--quiet", &revision])?
        .status
        .success();

    let mut all: BTreeSet<String> = BTreeSet::new();
    if base_exists {
        let diff = git_stdout(
            root,
            &["diff", "--name-only", "-z", "--relative", base, "--"],
        )?;
        all.extend(names(&diff));
    } else if base == "HEAD" {
        all.extend(names(&git_stdout(root, &["ls-files", "-z", "--cached"])?));
    } else {
        bail!("unknown git ref `{base}`");
    }
    let untracked = git_stdout(root, &["ls-files", "-z", "--others", "--exclude-standard"])?;
    all.extend(names(&untracked));

    let mut changes = Changes::default();
    for name in all.into_iter().filter(|n| n.ends_with(".md")) {
        if base_exists {
            let old = git(root, &["show", &format!("{base}:./{name}")])?;
            if old.status.success() {
                let text = String::from_utf8_lossy(&old.stdout);
                if let (Some(node), _) = parse_file_text(Path::new(&name), &text) {
                    changes.old_ids.insert(node.id);
                }
            }
        }
        changes.paths.insert(PathBuf::from(name));
    }
    Ok(changes)
}
