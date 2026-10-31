//! `oxispec status`: what is in the project and what needs attention.

use std::path::Path;

use anyhow::Result;
use oxispec_core::status::{self, Summary};
use oxispec_core::{Graph, OUTPUT_SCHEMA_VERSION, check, config, index};

use crate::git;

pub fn status_command(root: &Path, at: Option<&str>, json: bool, cache: bool) -> Result<()> {
    config::load(root)?;
    let revision = at.map(|spec| git::resolve_rev(root, spec)).transpose()?;
    let graph: Graph = match &revision {
        Some(rev) => git::graph_at(root, rev)?,
        None => index::load(root, cache).0,
    };
    let summary = status::summarize(&graph, &check::check(&graph));

    if json {
        let mut out = serde_json::to_value(&summary)?;
        out["schema_version"] = OUTPUT_SCHEMA_VERSION.into();
        out["at"] = serde_json::to_value(revision.as_ref().map(crate::revision_json))?;
        outln!("{}", serde_json::to_string_pretty(&out)?);
        return Ok(());
    }
    if let Some(rev) = &revision {
        outln!("reading {rev}");
    }
    print_text(&summary);
    Ok(())
}

fn print_text(s: &Summary) {
    outln!("{} nodes, {} links", s.nodes, s.links);
    for (ty, count) in &s.types {
        let breakdown = s
            .statuses
            .get(ty)
            .map(|by| {
                let parts: Vec<String> = by.iter().map(|(st, n)| format!("{n} {st}")).collect();
                format!(" ({})", parts.join(", "))
            })
            .unwrap_or_default();
        outln!("  {ty:<14} {count}{breakdown}");
    }
    if s.requirements > 0 {
        let percent =
            (s.requirements_with_claims as f64 * 1000.0 / s.requirements as f64).round() / 10.0;
        outln!(
            "{} scenarios; {} of {} requirements have claims ({percent}%)",
            s.scenarios,
            s.requirements_with_claims,
            s.requirements
        );
    }
    if s.errors + s.warnings == 0 {
        outln!("checks: clean");
    } else {
        let codes: Vec<String> = s.codes.iter().map(|(c, n)| format!("{c} x{n}")).collect();
        outln!(
            "checks: {} errors, {} warnings ({}); `oxispec check` lists them",
            s.errors,
            s.warnings,
            codes.join(", ")
        );
    }
    if !s.changes_in_flight.is_empty() {
        outln!("changes in flight: {}", s.changes_in_flight.join(", "));
    }
    if let Some(version) = &s.latest_release {
        outln!("latest release: {version}");
    }
}
