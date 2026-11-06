//! `oxispec risks`: the risk register, and moving a risk along.

use std::fs;
use std::path::Path;

use anyhow::{Context, Result, bail};
use oxispec_core::debt;
use oxispec_core::edit::set_fields;
use oxispec_core::risks::{self, LEVELS, STATUSES};
use oxispec_core::{OUTPUT_SCHEMA_VERSION, config};
use serde_json::json;

use crate::{git, load_graph, require_module};

fn one_of(what: &str, value: &str, allowed: &[&str]) -> Result<()> {
    if !allowed.contains(&value) {
        bail!("{what} `{value}` is not one of {}", allowed.join(", "));
    }
    Ok(())
}

pub fn list(
    root: &Path,
    status: Option<&str>,
    level: Option<&str>,
    all: bool,
    json: bool,
    cache: bool,
) -> Result<()> {
    let cfg = config::load(root)?;
    require_module(&cfg, "risks")?;
    if let Some(status) = status {
        one_of("status", status, STATUSES)?;
    }
    if let Some(level) = level {
        one_of("level", level, LEVELS)?;
    }
    let graph = load_graph(root, cache);
    let today = debt::today();
    let items: Vec<_> = risks::register(&graph, &today, all || status.is_some())
        .into_iter()
        .filter(|i| status.is_none_or(|s| i.node.status.as_deref() == Some(s)))
        .filter(|i| level.is_none_or(|l| i.level == Some(l)))
        .collect();
    let critical = risks::critical(&graph);
    if json {
        let rows: Vec<_> = items
            .iter()
            .map(|i| {
                let n = i.node;
                json!({
                    "id": n.id,
                    "title": n.title,
                    "status": n.status,
                    "likelihood": n.likelihood,
                    "impact": n.impact,
                    "level": i.level,
                    "owner": n.owner,
                    "date": n.date,
                    "review": n.review,
                    "approved_by": n.approved_by,
                    "mitigated_by": n.links.get("mitigated-by").cloned().unwrap_or_default(),
                    "arises_from": n.links.get("arises-from").cloned().unwrap_or_default(),
                    "path": n.path,
                    "live": i.live,
                    "age_days": i.age_days,
                    "review_overdue": i.review_overdue,
                })
            })
            .collect();
        outln!(
            "{}",
            serde_json::to_string_pretty(&json!({
                "schema_version": OUTPUT_SCHEMA_VERSION,
                "today": today,
                "risks": rows,
                "critical": critical.iter().map(|n| &n.id).collect::<Vec<_>>(),
            }))?
        );
        return Ok(());
    }
    if items.is_empty() {
        outln!("no risks");
    } else {
        outln!(
            "{:<10} {:<9} {:<11} {:<8} {:<8} {:<10}  TITLE",
            "ID",
            "LEVEL",
            "STATUS",
            "LIKELY",
            "IMPACT",
            "OWNER"
        );
    }
    for i in &items {
        let n = i.node;
        outln!(
            "{:<10} {:<9} {:<11} {:<8} {:<8} {:<10}  {}{}",
            n.id,
            i.level.unwrap_or("-"),
            n.status.as_deref().unwrap_or("-"),
            n.likelihood.as_deref().unwrap_or("-"),
            n.impact.as_deref().unwrap_or("-"),
            n.owner.as_deref().unwrap_or("-"),
            n.title.as_deref().unwrap_or(""),
            if i.review_overdue {
                "  [review overdue]"
            } else {
                ""
            }
        );
    }
    let live = items.iter().filter(|i| i.live).count();
    outln!("{live} live");
    if !critical.is_empty() {
        outln!(
            "critical and live, which holds a release back: {}",
            critical
                .iter()
                .map(|n| n.id.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        );
    }
    Ok(())
}

/// Moves a risk to a status and records that it was looked at today. An
/// accepted risk records who accepted it.
// spec: REQ-RISK-0003
pub fn set_status(
    root: &Path,
    id: &str,
    status: &str,
    by: Option<&str>,
    cache: bool,
) -> Result<()> {
    let cfg = config::load(root)?;
    require_module(&cfg, "risks")?;
    one_of("status", status, STATUSES)?;
    let graph = load_graph(root, cache);
    let node = match graph.get(id) {
        Some(n) if n.ty == "risk" => n,
        Some(n) => bail!("{id} is a {}, not a risk", n.ty),
        None => bail!("no node {id}"),
    };
    let path = root.join(&node.path);
    let text = fs::read_to_string(&path)?;
    let today = debt::today();
    let who = if status == "accepted" {
        Some(match by {
            Some(b) => b.to_string(),
            None => git::user_name(root)?
                .context("say who accepts the risk with --by NAME, or set git user.name")?,
        })
    } else {
        None
    };
    let mut updates: Vec<(&str, Option<&str>)> =
        vec![("status", Some(status)), ("date", Some(&today))];
    if let Some(w) = who.as_deref() {
        updates.push(("approved_by", Some(w)));
        updates.push(("approved_on", Some(&today)));
    } else if node.approved_by.is_some() {
        updates.push(("approved_by", None));
        updates.push(("approved_on", None));
    }
    let updated =
        set_fields(&text, &updates).map_err(|e| anyhow::anyhow!("{}: {e}", node.path.display()))?;
    fs::write(&path, updated)?;
    match who {
        Some(w) => outln!("{id} is now {status}, accepted by {w} (reviewed {today})"),
        None => outln!("{id} is now {status} (reviewed {today})"),
    }
    Ok(())
}

pub const TEMPLATE: &str = "---
id: @ID@
type: risk
status: open
likelihood: @LIKELIHOOD@
impact: @IMPACT@
owner:
date: @DATE@
review:
@LINKS@---
# @ID@: @TITLE@

## The risk

What could go wrong, and to what.

## What would trigger it

## What it would cost

## What is done about it

Link what mitigates it with `mitigated-by`.
";
