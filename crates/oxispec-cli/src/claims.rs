//! `oxispec claims`: the vocabulary and the claims written against it.

use std::collections::BTreeMap;
use std::path::Path;
use std::process::ExitCode;

use anyhow::{Result, bail};
use oxispec_core::claims::{self, Claim};
use oxispec_core::{Graph, OUTPUT_SCHEMA_VERSION, config, index};

use crate::git;

pub fn claims_command(
    root: &Path,
    variable: Option<&str>,
    at: Option<&str>,
    json: bool,
    cache: bool,
) -> Result<ExitCode> {
    let cfg = config::load(root)?;
    if !cfg.has("consistency") {
        bail!("module `consistency` is not enabled; run `oxispec modules add consistency`");
    }
    let revision = at.map(|spec| git::resolve_rev(root, spec)).transpose()?;
    let graph: Graph = match &revision {
        Some(rev) => git::graph_at(root, rev)?,
        None => index::load(root, cache).0,
    };
    let analysis = claims::analyze(&graph);
    if let Some(name) = variable
        && !analysis.vocabulary.variables.contains_key(name)
    {
        bail!("no variable `{name}` in the vocabulary (`oxispec claims` lists them)");
    }

    let mut by_variable: BTreeMap<&str, Vec<&Claim>> = BTreeMap::new();
    for name in analysis.vocabulary.variables.keys() {
        by_variable.entry(name).or_default();
    }
    for claim in &analysis.claims {
        by_variable.entry(&claim.subject).or_default().push(claim);
    }
    let shown = |name: &str| variable.is_none_or(|v| v == name);

    let requirements = graph.nodes.iter().filter(|n| n.ty == "requirement").count();
    let with_claims = graph
        .nodes
        .iter()
        .filter(|n| n.ty == "requirement" && !n.claims.is_empty())
        .count();
    let percent = if requirements == 0 {
        100.0
    } else {
        (with_claims as f64 * 1000.0 / requirements as f64).round() / 10.0
    };

    if json {
        let variables: Vec<_> = by_variable
            .iter()
            .filter(|(name, _)| shown(name))
            .map(|(name, list)| {
                let var = analysis.vocabulary.variables.get(*name);
                serde_json::json!({
                    "name": name,
                    "type": var.map(|v| v.kind.name()),
                    "values": var.map(|v| &v.values),
                    "declared_in": var.map(|v| &v.node),
                    "claims": list.iter().map(|c| serde_json::json!({
                        "node": c.node,
                        "path": c.path,
                        "line": c.line,
                        "modality": c.modality.name(),
                        "scope": c.scope_text,
                        "active": c.active,
                        "text": c.text,
                    })).collect::<Vec<_>>(),
                })
            })
            .collect();
        let out = serde_json::json!({
            "schema_version": OUTPUT_SCHEMA_VERSION,
            "at": revision.as_ref().map(crate::revision_json),
            "variables": variables,
            "requirements": requirements,
            "requirements_with_claims": with_claims,
            "claims_percent": percent,
        });
        println!("{}", serde_json::to_string_pretty(&out)?);
        return Ok(ExitCode::SUCCESS);
    }

    if let Some(rev) = &revision {
        println!("reading {rev}");
    }
    if analysis.vocabulary.variables.is_empty() {
        println!(
            "no variables yet: declare them in a `vocabulary` block (`oxispec new vocabulary \"Name\"`)"
        );
    }
    for (name, list) in by_variable.iter().filter(|(name, _)| shown(name)) {
        match analysis.vocabulary.variables.get(*name) {
            Some(var) => {
                let values = if var.values.is_empty() {
                    String::new()
                } else {
                    format!(": {}", var.values.join(", "))
                };
                println!(
                    "{name} ({}{values}), declared in {}",
                    var.kind.name(),
                    var.node
                );
            }
            None => println!("{name}"),
        }
        if list.is_empty() {
            println!("  no claims");
        }
        for claim in list {
            let note = if claim.active { "" } else { "  (not counted)" };
            println!(
                "  {:<14} {:<10} {}{note}",
                claim.node,
                claim.modality.name(),
                claim.text
            );
        }
    }
    println!(
        "{with_claims} of {requirements} requirements have claims ({percent}%); `oxispec check` reports conflicts"
    );
    Ok(ExitCode::SUCCESS)
}
