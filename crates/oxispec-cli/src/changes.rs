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
        outln!("{} is already {to}", node.id);
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
    outln!("{}: {from_text} -> {to}{approval}", node.id);
    Ok(())
}

/// Ticks (`done`) or unticks the tasks of a change set: the checkbox on the line of each task.
// spec: REQ-CHANGE-0006
pub fn change_task(
    root: &Path,
    id: &str,
    done: bool,
    wanted: &[String],
    all: bool,
    cache: bool,
) -> Result<()> {
    config::load(root)?;
    let graph = index::load(root, cache).0;
    let node = graph
        .get(id)
        .with_context(|| format!("no node with id {id}"))?;
    if node.ty != "change" {
        bail!("{} is a {}, not a change set", node.id, node.ty);
    }
    if node.tasks.is_empty() {
        bail!("{} has no tasks under `## Tasks`", node.id);
    }
    if wanted.is_empty() && !all {
        bail!("say which tasks: their numbers (`1.2`) or places (`#3`), or --all");
    }
    // The tasks to change, by position in the list.
    let mut picked: Vec<usize> = Vec::new();
    if all {
        picked.extend(0..node.tasks.len());
    }
    for w in wanted {
        let found: Vec<usize> = if let Some(n) = w.strip_prefix('#') {
            n.parse::<usize>()
                .ok()
                .filter(|n| (1..=node.tasks.len()).contains(n))
                .map(|n| vec![n - 1])
                .unwrap_or_default()
        } else {
            node.tasks
                .iter()
                .enumerate()
                .filter(|(_, t)| t.number.as_deref() == Some(w.trim_end_matches('.')))
                .map(|(i, _)| i)
                .collect()
        };
        match found.len() {
            0 => bail!(
                "{} has no task `{w}`; its tasks are {}",
                node.id,
                node.tasks
                    .iter()
                    .enumerate()
                    .map(|(i, t)| t.number.clone().unwrap_or_else(|| format!("#{}", i + 1)))
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
            1 => picked.push(found[0]),
            _ => bail!(
                "{} has more than one task `{w}`; use its place in the list (`#N`) instead",
                node.id
            ),
        }
    }
    picked.sort_unstable();
    picked.dedup();
    let path = root.join(&node.path);
    let text =
        fs::read_to_string(&path).with_context(|| format!("cannot read {}", path.display()))?;
    let mut lines: Vec<String> = text.split('\n').map(str::to_string).collect();
    let mut changed = Vec::new();
    for &i in &picked {
        let task = &node.tasks[i];
        if task.done == done {
            continue;
        }
        let line = lines.get_mut(task.line - 1).with_context(|| {
            format!(
                "line {} of {} is not a task any more; run the command again",
                task.line,
                node.path.display()
            )
        })?;
        let boxes: &[&str] = if done { &["[ ]"] } else { &["[x]", "[X]"] };
        let Some(found) = boxes.iter().find(|b| line.contains(**b)) else {
            bail!(
                "line {} of {} is not a task any more; run the command again",
                task.line,
                node.path.display()
            );
        };
        *line = line.replacen(found, if done { "[x]" } else { "[ ]" }, 1);
        changed.push(i);
    }
    if !changed.is_empty() {
        fs::write(&path, lines.join("\n"))
            .with_context(|| format!("cannot write {}", path.display()))?;
    }
    let (was_done, total) = node.progress();
    let now = if done {
        was_done + changed.len()
    } else {
        was_done - changed.len()
    };
    for &i in &changed {
        let t = &node.tasks[i];
        outln!(
            "{}: {} {}",
            node.id,
            t.number.clone().unwrap_or_else(|| format!("#{}", i + 1)),
            if done { "done" } else { "not done" }
        );
    }
    if changed.is_empty() {
        outln!(
            "{}: nothing to change, the tasks are already {}",
            node.id,
            if done { "done" } else { "not done" }
        );
    }
    outln!("{}: {now}/{total} tasks done", node.id);
    Ok(())
}

// spec: REQ-CHANGE-0004
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
                    "tasks": {"done": n.progress().0, "total": n.progress().1},
                })
            })
            .collect();
        let out = json!({"schema_version": OUTPUT_SCHEMA_VERSION, "change_sets": list});
        outln!("{}", serde_json::to_string_pretty(&out)?);
        return Ok(());
    }
    if changes.is_empty() {
        outln!(
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
        let (done, total) = n.progress();
        let tasks = if total == 0 {
            String::new()
        } else {
            format!("  [{done}/{total} tasks]")
        };
        outln!(
            "{:<12} {:<10} {}{tasks}{approval}",
            n.id,
            n.status.as_deref().unwrap_or("no status"),
            n.title.as_deref().unwrap_or("")
        );
    }
    Ok(())
}
