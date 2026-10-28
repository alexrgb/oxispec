//! `oxispec change`: the lifecycle of change sets, and a list of them.

use std::fs;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result, bail};
use oxispec_core::change::{self, STATUSES};
use oxispec_core::edit::set_fields;
use oxispec_core::{OUTPUT_SCHEMA_VERSION, config, index};
use serde_json::json;

use crate::git;

/// Moves a change set along its lifecycle, recording who approved it.
///
/// This writes down an approval, it does not enforce who may give one. That
/// waits for access control. Until then, review rules on the repository are
/// what stops someone approving their own change.
pub fn change_status(
    root: &Path,
    id: &str,
    to: &str,
    by: Option<&str>,
    force: bool,
    cache: bool,
) -> Result<()> {
    config::load(root)?;
    if !STATUSES.contains(&to) {
        bail!("unknown status `{to}` (known: {})", STATUSES.join(", "));
    }
    let graph = index::load(root, cache).0;
    let node = graph
        .get(id)
        .with_context(|| format!("no node with id {id}"))?;
    if node.ty != "change" {
        bail!("{} is a {}, not a change set", node.id, node.ty);
    }
    let from = node.status.clone().unwrap_or_default();
    if from == to {
        println!("{} is already {to}", node.id);
        return Ok(());
    }
    let allowed = change::next_statuses(&from);
    if !force && !allowed.contains(&to) {
        if allowed.is_empty() {
            bail!(
                "a change set that is {from:?} cannot move to {to}; use --force to set it anyway"
            );
        }
        bail!(
            "a change set cannot go from {from} to {to}; from {from} it can go to {}. Use --force to skip a step.",
            allowed.join(" or ")
        );
    }

    let mut updates: Vec<(&str, Option<String>)> = vec![("status", Some(to.to_string()))];
    let mut approval = String::new();
    if change::needs_approval(to) {
        // Approving records who. Merging keeps the approval that is already there.
        if to == "approved" || node.approved_by.is_none() {
            let who = match by {
                Some(name) => name.to_string(),
                None => git::user_name(root)?
                    .context("say who approves with --by NAME, or set git user.name")?,
            };
            let today = change::utc_date(
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .map_or(0, |d| d.as_secs()),
            );
            approval = format!(" (approved by {who} on {today})");
            updates.push(("approved_by", Some(who)));
            updates.push(("approved_on", Some(today)));
        }
    } else if change::withdraws_approval(to) {
        updates.push(("approved_by", None));
        updates.push(("approved_on", None));
        if node.approved_by.is_some() {
            approval = " (the approval is withdrawn)".to_string();
        }
    }

    let path = root.join(&node.path);
    let text =
        fs::read_to_string(&path).with_context(|| format!("cannot read {}", path.display()))?;
    let borrowed: Vec<(&str, Option<&str>)> =
        updates.iter().map(|(k, v)| (*k, v.as_deref())).collect();
    let updated = set_fields(&text, &borrowed)
        .with_context(|| format!("cannot edit {}", node.path.display()))?;
    fs::write(&path, updated).with_context(|| format!("cannot write {}", path.display()))?;
    let from_text = if from.is_empty() { "no status" } else { &from };
    println!("{}: {from_text} -> {to}{approval}", node.id);
    Ok(())
}

pub fn change_list(
    root: &Path,
    status: Option<&str>,
    json_output: bool,
    cache: bool,
) -> Result<()> {
    config::load(root)?;
    if let Some(s) = status
        && !STATUSES.contains(&s)
    {
        bail!("unknown status `{s}` (known: {})", STATUSES.join(", "));
    }
    let graph = index::load(root, cache).0;
    let changes: Vec<_> = graph
        .nodes
        .iter()
        .filter(|n| n.ty == "change")
        .filter(|n| status.is_none_or(|s| n.status.as_deref() == Some(s)))
        .collect();
    if json_output {
        let list: Vec<_> = changes
            .iter()
            .map(|n| {
                json!({
                    "id": n.id,
                    "status": n.status,
                    "title": n.title,
                    "path": n.path,
                    "approved_by": n.approved_by,
                    "approved_on": n.approved_on,
                    "changes": n.links.get("changes").cloned().unwrap_or_default(),
                })
            })
            .collect();
        let out = json!({"schema_version": OUTPUT_SCHEMA_VERSION, "change_sets": list});
        println!("{}", serde_json::to_string_pretty(&out)?);
        return Ok(());
    }
    if changes.is_empty() {
        println!(
            "no change sets{}",
            status.map_or(String::new(), |s| format!(" that are {s}"))
        );
    }
    for n in changes {
        let approval = match (&n.approved_by, &n.approved_on) {
            (Some(by), Some(on)) => format!("  approved by {by} on {on}"),
            (Some(by), None) => format!("  approved by {by}"),
            _ => String::new(),
        };
        println!(
            "{:<12} {:<10} {}{approval}",
            n.id,
            n.status.as_deref().unwrap_or("no status"),
            n.title.as_deref().unwrap_or("")
        );
    }
    Ok(())
}
