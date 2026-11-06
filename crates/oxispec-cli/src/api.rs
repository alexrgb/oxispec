//! `oxispec api show`: everything the project says about an operation.

use std::collections::BTreeSet;
use std::path::Path;

use anyhow::{Result, bail};
use oxispec_core::api::{self, Card};
use oxispec_core::trace::{coverage, scan_tree_with};
use oxispec_core::{OUTPUT_SCHEMA_VERSION, config, results};
use serde_json::json;

use crate::{load_graph, require_module};

/// How a scenario stands: from the tests and the imported results, when the
/// `tests` module is on.
fn standing(root: &Path, cache: bool) -> Result<Option<(BTreeSet<String>, BTreeSet<String>)>> {
    let cfg = config::load(root)?;
    if !cfg.has("tests") {
        return Ok(None);
    }
    let scan = scan_tree_with(root, &cfg.trace)?;
    let graph = load_graph(root, cache);
    let unverified = coverage(&graph, &scan.annotations, None)
        .unverified
        .into_iter()
        .collect();
    let failing = match results::load(root)? {
        Some(found) => results::scenario_status(&graph, &scan.annotations, &found)
            .into_iter()
            .filter(|(_, s)| *s == results::Status::Failing)
            .map(|(s, _)| s)
            .collect(),
        None => BTreeSet::new(),
    };
    Ok(Some((unverified, failing)))
}

fn state(id: &str, known: &Option<(BTreeSet<String>, BTreeSet<String>)>) -> &'static str {
    match known {
        None => "unknown (module tests is off)",
        Some((_, failing)) if failing.contains(id) => "failing",
        Some((unverified, _)) if unverified.contains(id) => "no test",
        Some(_) => "verified",
    }
}

fn card_json(c: &Card, known: &Option<(BTreeSet<String>, BTreeSet<String>)>) -> serde_json::Value {
    let o = c.operation;
    json!({
        "call": o.call(),
        "tag": o.tag,
        "summary": o.summary,
        "requirement": o.node,
        "status": c.status,
        "responses": o.responses,
        "scenarios": c.scenarios.iter().map(|s| json!({"id": s, "state": state(s, known)})).collect::<Vec<_>>(),
        "prds": c.prds,
        "decisions": c.decisions,
        "debt": c.debt,
        "risks": c.risks.iter().map(|(id, level)| json!({"id": id, "level": level})).collect::<Vec<_>>(),
        "test_plans": c.plans,
    })
}

pub fn show(root: &Path, target: &str, json_output: bool, cache: bool) -> Result<()> {
    let cfg = config::load(root)?;
    require_module(&cfg, "specs")?;
    let graph = load_graph(root, cache);
    let ops = api::operations(&graph, root)?;
    let chosen = api::select(&ops, target);
    if chosen.is_empty() {
        let known: Vec<String> = ops.iter().map(api::Operation::call).collect();
        bail!(
            "no operation matches `{target}`; the requirements state {}",
            if known.is_empty() {
                "none (a requirement block titled `GET /path` is an operation)".to_string()
            } else {
                known.join(", ")
            }
        );
    }
    let known = standing(root, cache)?;
    let cards: Vec<Card> = chosen.iter().map(|o| api::card(&graph, o)).collect();
    if json_output {
        outln!(
            "{}",
            serde_json::to_string_pretty(&json!({
                "schema_version": OUTPUT_SCHEMA_VERSION,
                "operations": cards.iter().map(|c| card_json(c, &known)).collect::<Vec<_>>(),
            }))?
        );
        return Ok(());
    }
    for c in &cards {
        let o = c.operation;
        outln!("{}  [{}]", o.call(), o.tag);
        if !o.summary.is_empty() {
            outln!("    {}", o.summary);
        }
        outln!(
            "    requirement {} ({})",
            o.node,
            c.status.as_deref().unwrap_or("no status")
        );
        let list = |label: &str, items: &[String]| {
            if !items.is_empty() {
                outln!("    {label}: {}", items.join(", "));
            }
        };
        list("PRD", &c.prds);
        list("decisions", &c.decisions);
        list("tech debt", &c.debt);
        let risks: Vec<String> = c
            .risks
            .iter()
            .map(|(id, level)| format!("{id} ({})", level.unwrap_or("no level")))
            .collect();
        list("risks", &risks);
        list("test plans", &c.plans);
        for (code, what) in &o.responses {
            outln!(
                "    responds {code}{}",
                if what.is_empty() {
                    String::new()
                } else {
                    format!(": {what}")
                }
            );
        }
        for s in &c.scenarios {
            outln!("    {s}: {}", state(s, &known));
        }
    }
    Ok(())
}
