//! `oxispec debt`: the tech debt backlog, and moving an item along.

use std::fs;
use std::path::Path;

use anyhow::{Result, bail};
use oxispec_core::debt::{self, SEVERITIES, STATUSES};
use oxispec_core::edit::set_fields;
use oxispec_core::{OUTPUT_SCHEMA_VERSION, config};
use serde_json::json;

use crate::{load_graph, require_module};

fn one_of(what: &str, value: &str, allowed: &[&str]) -> Result<()> {
    if !allowed.contains(&value) {
        bail!("{what} `{value}` is not one of {}", allowed.join(", "));
    }
    Ok(())
}

pub fn list(
    root: &Path,
    status: Option<&str>,
    severity: Option<&str>,
    all: bool,
    json: bool,
    cache: bool,
) -> Result<()> {
    let cfg = config::load(root)?;
    require_module(&cfg, "debt")?;
    if let Some(status) = status {
        one_of("status", status, STATUSES)?;
    }
    if let Some(severity) = severity {
        one_of("severity", severity, SEVERITIES)?;
    }
    let graph = load_graph(root, cache);
    let today = debt::today();
    // Asking for a status means asking for it even when it is closed.
    let items: Vec<_> = debt::backlog(&graph, &today, all || status.is_some())
        .into_iter()
        .filter(|i| status.is_none_or(|s| i.node.status.as_deref() == Some(s)))
        .filter(|i| severity.is_none_or(|s| i.node.severity.as_deref() == Some(s)))
        .collect();
    let counts = debt::live_counts(&graph);
    let over = debt::over_limits(&graph);
    if json {
        let rows: Vec<_> = items
            .iter()
            .map(|i| {
                let n = i.node;
                json!({
                    "id": n.id,
                    "title": n.title,
                    "status": n.status,
                    "severity": n.severity,
                    "owner": n.owner,
                    "date": n.date,
                    "due": n.due,
                    "cost": n.cost,
                    "interest": n.interest,
                    "arises_from": n.links.get("arises-from").cloned().unwrap_or_default(),
                    "path": n.path,
                    "live": i.live,
                    "age_days": i.age_days,
                    "overdue": i.overdue,
                    "stale": i.stale,
                })
            })
            .collect();
        let limits: Vec<_> = graph
            .debt_config()
            .limits
            .iter()
            .map(|(severity, limit)| {
                json!({"severity": severity, "limit": limit,
                       "live": counts.get(severity).copied().unwrap_or(0)})
            })
            .collect();
        outln!(
            "{}",
            serde_json::to_string_pretty(&json!({
                "schema_version": OUTPUT_SCHEMA_VERSION,
                "today": today,
                "items": rows,
                "live": counts,
                "limits": limits,
                "over_limits": over.iter().map(|(s, _, _)| s).collect::<Vec<_>>(),
            }))?
        );
        return Ok(());
    }
    if items.is_empty() {
        outln!("no debt items");
    } else {
        outln!(
            "{:<11} {:<9} {:<12} {:>5}  {:<10}  {:<10}  TITLE",
            "ID",
            "SEVERITY",
            "STATUS",
            "AGE",
            "DUE",
            "OWNER"
        );
    }
    for i in &items {
        let n = i.node;
        let age = i.age_days.map_or("-".to_string(), |a| format!("{a}d"));
        let flags = format!(
            "{}{}",
            if i.overdue { "  [overdue]" } else { "" },
            if i.stale { "  [stale]" } else { "" }
        );
        outln!(
            "{:<11} {:<9} {:<12} {:>5}  {:<10}  {:<10}  {}{flags}",
            n.id,
            n.severity.as_deref().unwrap_or("-"),
            n.status.as_deref().unwrap_or("-"),
            age,
            n.due.as_deref().unwrap_or("-"),
            n.owner.as_deref().unwrap_or("-"),
            n.title.as_deref().unwrap_or("")
        );
    }
    let live: usize = counts.values().sum();
    let by_severity: Vec<String> = SEVERITIES
        .iter()
        .rev()
        .filter_map(|s| counts.get(*s).map(|n| format!("{n} {s}")))
        .collect();
    outln!(
        "{live} still to be paid{}",
        if by_severity.is_empty() {
            String::new()
        } else {
            format!(": {}", by_severity.join(", "))
        }
    );
    for (severity, live, limit) in &over {
        outln!("over the limit: {live} {severity}, and the limit is {limit}");
    }
    Ok(())
}

/// Moves an item to a status and records that it was looked at today.
pub fn set_status(root: &Path, id: &str, status: &str, cache: bool) -> Result<()> {
    let cfg = config::load(root)?;
    require_module(&cfg, "debt")?;
    one_of("status", status, STATUSES)?;
    let graph = load_graph(root, cache);
    let node = match graph.get(id) {
        Some(n) if n.ty == "debt" => n,
        Some(n) => bail!("{id} is a {}, not a debt item", n.ty),
        None => bail!("no node {id}"),
    };
    let path = root.join(&node.path);
    let text = fs::read_to_string(&path)?;
    let today = debt::today();
    let updated = set_fields(&text, &[("status", Some(status)), ("date", Some(&today))])
        .map_err(|e| anyhow::anyhow!("{}: {e}", node.path.display()))?;
    fs::write(&path, updated)?;
    outln!("{id} is now {status} (reviewed {today})");
    Ok(())
}

pub const TEMPLATE: &str = "---
id: @ID@
type: debt
status: open
severity: @SEVERITY@
owner:
date: @DATE@
due:
cost:
interest:
@LINKS@---
# @ID@: @TITLE@

## What it is

The shortcut, and where it is.

## Why it was taken

What was gained, and what was known at the time.

## What paying it off takes

The steps, and what could go wrong.
";
