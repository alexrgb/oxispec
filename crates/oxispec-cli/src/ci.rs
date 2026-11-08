//! `oxispec ci`: what a change introduces, for pull request checks.
//!
//! The specs in the working tree are compared with the specs where the
//! branch left the base. Only findings that the change introduced count, so
//! a project with old problems can still gate new work.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use anyhow::{Context, Result};
use oxispec_core::diff::diff_graphs;
use oxispec_core::impact::impact;
use oxispec_core::release::summarize;
use oxispec_core::report::{Report, Touched, junit, markdown, sarif};
use oxispec_core::suspect;
use oxispec_core::trace::scan_tree_with;
use oxispec_core::{OUTPUT_SCHEMA_VERSION, Severity, check, config, index};
use serde_json::json;

use crate::git;

pub struct CiOptions {
    pub base: String,
    pub strict: bool,
    pub json: bool,
    pub sarif: Option<PathBuf>,
    pub junit: Option<PathBuf>,
    pub markdown: Option<PathBuf>,
    pub cache: bool,
}

fn write_report(path: &Path, text: &str) -> Result<()> {
    if let Some(dir) = path.parent().filter(|d| !d.as_os_str().is_empty()) {
        fs::create_dir_all(dir).with_context(|| format!("cannot create {}", dir.display()))?;
    }
    fs::write(path, text).with_context(|| format!("cannot write {}", path.display()))
}

// spec: REQ-CI-0001
// spec: REQ-CI-0002
// spec: REQ-CI-0003
// spec: REQ-CI-0004
// spec: REQ-SUSPECT-0002
// spec: REQ-ENTITY-0005
pub fn ci_command(root: &Path, options: &CiOptions) -> Result<ExitCode> {
    let cfg = config::load(root)?;
    let base = git::merge_base(root, &options.base)?;
    let old = git::graph_at(root, &base)?;
    let new = index::load(root, options.cache).0;
    let mut diff = diff_graphs(&old, &new);

    // A link between code and a spec that changed since it was confirmed is
    // always reported, as it would be to anyone running `oxispec suspect`.
    if cfg.has("tests") {
        let scan = scan_tree_with(root, &cfg.trace)?;
        let lock = suspect::load(root)?;
        let links = suspect::links(&new, &scan.annotations, &lock);
        diff.new_findings.extend(suspect::findings(&links));
    }

    let changes = git::changes(root, &base.oid)?;
    let reach = impact(&new, &changes.paths, &changes.old_ids);
    let touched: Vec<Touched> = new
        .nodes
        .iter()
        .filter(|n| reach.paths.contains(&n.path) || reach.ids.contains(&n.id))
        .map(|n| Touched {
            id: n.id.clone(),
            ty: n.ty.clone(),
            path: n.path.clone(),
        })
        .collect();
    // What an entity that changed leaves depending on it. It is information, not a finding:
    // whether the dependents are still right is for a person to say.
    let entity_impacts = if cfg.has("consistency") {
        oxispec_core::entities::impacts(&old, &new, oxispec_core::entities::DEFAULT_DEPTH)
    } else {
        Vec::new()
    };
    let reportable = check(&new)
        .iter()
        .filter(|f| f.severity != Severity::Info)
        .count();
    let report = Report {
        base: base.to_string(),
        total_nodes: new.nodes.len(),
        touched,
        existing: reportable.saturating_sub(diff.new_findings.len()),
        bump: summarize(&new, &diff).bump(),
        diff: &diff,
        strict: options.strict,
        entities: entity_impacts,
    };

    if let Some(path) = &options.markdown {
        write_report(path, &markdown(&report))?;
    }
    if let Some(path) = &options.sarif {
        let text = serde_json::to_string_pretty(&sarif(&report, env!("CARGO_PKG_VERSION")))?;
        write_report(path, &text)?;
    }
    if let Some(path) = &options.junit {
        write_report(path, &junit(&report))?;
    }

    let (errors, warnings) = (report.new_errors(), report.new_warnings());
    if options.json {
        let out = json!({
            "schema_version": OUTPUT_SCHEMA_VERSION,
            "base": crate::revision_json(&base),
            "nodes": report.total_nodes,
            "touched": report.touched.iter().map(|t| json!({"id": t.id, "type": t.ty, "path": t.path})).collect::<Vec<_>>(),
            "added": diff.added,
            "removed": diff.removed,
            "changed": diff.changed,
            "entities": report.entities,
            "bump": report.bump,
            "new_findings": diff.new_findings,
            "fixed_findings": diff.fixed_findings,
            "existing_findings": report.existing,
            "errors": errors,
            "warnings": warnings,
            "failed": report.failed(),
        });
        outln!("{}", serde_json::to_string_pretty(&out)?);
    } else {
        outln!("comparing with {base}");
        for f in &diff.new_findings {
            outln!("{}", crate::finding_line(f));
            if let Some(path) = &f.path {
                match f.line {
                    Some(line) => outln!("  --> {}:{line}", path.display()),
                    None => outln!("  --> {}", path.display()),
                }
            }
        }
        for e in &report.entities {
            let breaking = e.changes.iter().any(|c| c.breaking);
            let what: Vec<&str> = e.changes.iter().map(|c| c.detail.as_str()).collect();
            outln!(
                "entity {} ({}){}: {}; {} depend on it",
                e.id,
                e.name,
                if breaking { " [breaking]" } else { "" },
                what.join("; "),
                e.affected.len()
            );
        }
        outln!(
            "{} nodes changed, {} touched: {errors} new errors, {warnings} new warnings, {} fixed, {} existing not shown",
            diff.added.len() + diff.removed.len() + diff.changed.len(),
            report.touched.len(),
            diff.fixed_findings.len(),
            report.existing
        );
    }
    Ok(if report.failed() {
        ExitCode::from(1)
    } else {
        ExitCode::SUCCESS
    })
}
