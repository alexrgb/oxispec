//! `oxispec test-plan`: the plans, read against the tests and the risks.

use std::collections::BTreeSet;
use std::path::Path;
use std::process::ExitCode;

use anyhow::{Result, bail};
use oxispec_core::testplan::{self, PlanReport};
use oxispec_core::trace::{coverage, scan_tree_with};
use oxispec_core::{OUTPUT_SCHEMA_VERSION, config, index, results};
use serde_json::json;

fn row_json(r: &PlanReport) -> serde_json::Value {
    json!({
        "id": r.node.id,
        "title": r.node.title,
        "status": r.node.status,
        "requirements": r.requirements.iter().map(|q| json!({
            "id": q.id, "scenarios": q.scenarios,
            "unverified": q.unverified, "failing": q.failing,
        })).collect::<Vec<_>>(),
        "risks": r.risks.iter().map(|k| json!({
            "id": k.id, "level": k.level, "live": k.live,
            "mitigated_by": k.mitigations, "tested": k.tested,
        })).collect::<Vec<_>>(),
        "other": r.other,
        "gaps": r.has_gaps(),
    })
}

pub fn run(
    root: &Path,
    id: Option<&str>,
    strict: bool,
    json_output: bool,
    cache: bool,
) -> Result<ExitCode> {
    let cfg = config::load(root)?;
    if !cfg.has("tests") {
        bail!("module `tests` is not enabled; run `oxispec modules add tests`");
    }
    let scan = scan_tree_with(root, &cfg.trace)?;
    let graph = index::load(root, cache).0;
    let cov = coverage(&graph, &scan.annotations, None);
    let unverified: BTreeSet<String> = cov.unverified.iter().cloned().collect();
    let failing: BTreeSet<String> = match results::load(root)? {
        Some(found) => results::scenario_status(&graph, &scan.annotations, &found)
            .into_iter()
            .filter(|(_, s)| *s == results::Status::Failing)
            .map(|(s, _)| s)
            .collect(),
        None => BTreeSet::new(),
    };
    let mut reports = testplan::report(&graph, &unverified, &failing);
    if let Some(id) = id {
        match graph.get(id) {
            Some(n) if n.ty == "test-plan" => reports.retain(|r| r.node.id == n.id),
            Some(n) => bail!("{id} is a {}, not a test plan", n.ty),
            None => bail!("no node {id}"),
        }
    }
    let failed = strict
        && reports
            .iter()
            .any(|r| r.node.status.as_deref() == Some("approved") && r.has_gaps());
    if json_output {
        outln!(
            "{}",
            serde_json::to_string_pretty(&json!({
                "schema_version": OUTPUT_SCHEMA_VERSION,
                "plans": reports.iter().map(row_json).collect::<Vec<_>>(),
            }))?
        );
        return Ok(if failed {
            ExitCode::FAILURE
        } else {
            ExitCode::SUCCESS
        });
    }
    if reports.is_empty() {
        outln!("no test plans; write one with `oxispec new test-plan \"Title\"`");
    }
    for r in &reports {
        let n = r.node;
        outln!(
            "{} {} ({})",
            n.id,
            n.title.as_deref().unwrap_or(""),
            n.status.as_deref().unwrap_or("no status")
        );
        for q in &r.requirements {
            let state = if q.unverified.is_empty() && q.failing.is_empty() {
                format!("{} scenario(s), all verified", q.scenarios)
            } else {
                format!(
                    "{} scenario(s); not verified: {}; failing: {}",
                    q.scenarios,
                    if q.unverified.is_empty() {
                        "none".into()
                    } else {
                        q.unverified.join(", ")
                    },
                    if q.failing.is_empty() {
                        "none".into()
                    } else {
                        q.failing.join(", ")
                    }
                )
            };
            outln!("    {}: {state}", q.id);
        }
        for k in &r.risks {
            let state = match (k.mitigations.is_empty(), k.tested) {
                (true, _) => "nothing mitigates it yet".to_string(),
                (false, true) => format!("mitigation tested ({})", k.mitigations.join(", ")),
                (false, false) => {
                    format!("mitigation not fully tested ({})", k.mitigations.join(", "))
                }
            };
            outln!(
                "    {} [{}{}]: {state}",
                k.id,
                k.level.unwrap_or("no level"),
                if k.live { ", live" } else { "" }
            );
        }
        for o in &r.other {
            outln!("    {o}: covered, but it is not a requirement or a risk, so it is not read");
        }
    }
    Ok(if failed {
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    })
}
