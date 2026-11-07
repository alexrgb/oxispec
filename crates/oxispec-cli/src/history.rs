//! `oxispec history` and `oxispec diff`: how a node, and the whole set of
//! specs, changed over time. Revisions come from git.

use std::path::Path;

use anyhow::{Context, Result};
use oxispec_core::diff::{Change, diff_graphs, diff_nodes};
use oxispec_core::parse::parse_file_text;
use oxispec_core::{Node, OUTPUT_SCHEMA_VERSION, config, index};
use serde_json::json;

use crate::git::{self, Commit, Revision};

/// What happened to a node in one commit.
enum Event {
    Created,
    Deleted,
    Changed(Vec<Change>),
}

impl Event {
    fn between(old: Option<&Node>, new: Option<&Node>) -> Event {
        match (old, new) {
            (None, Some(_)) => Event::Created,
            (Some(_), None) => Event::Deleted,
            (Some(old), Some(new)) => Event::Changed(diff_nodes(old, new)),
            (None, None) => Event::Changed(Vec::new()),
        }
    }

    fn name(&self) -> &'static str {
        match self {
            Event::Created => "created",
            Event::Deleted => "deleted",
            Event::Changed(_) => "changed",
        }
    }

    fn describe(&self) -> String {
        match self {
            Event::Created => "created".to_string(),
            Event::Deleted => "deleted".to_string(),
            Event::Changed(changes) if changes.is_empty() => {
                "no change to the node itself".to_string()
            }
            Event::Changed(changes) => changes
                .iter()
                .map(Change::describe)
                .collect::<Vec<_>>()
                .join("; "),
        }
    }

    fn changes(&self) -> &[Change] {
        match self {
            Event::Changed(changes) => changes,
            _ => &[],
        }
    }
}

struct Entry {
    /// `None` for changes that are not committed yet.
    commit: Option<Commit>,
    event: Event,
}

fn revision_json(revision: &Revision) -> serde_json::Value {
    json!({"commit": revision.oid, "short": revision.short, "subject": revision.subject})
}

// spec: REQ-HISTORY-0001
// spec: REQ-HISTORY-0002
pub fn history_command(
    root: &Path,
    id: &str,
    at: Option<&str>,
    limit: usize,
    json_output: bool,
    cache: bool,
) -> Result<()> {
    config::load(root)?;
    git::require_repository(root, "history")?;
    let revision = at.map(|spec| git::resolve_rev(root, spec)).transpose()?;
    let graph = match &revision {
        Some(rev) => git::graph_at(root, rev)?,
        None => index::load(root, cache).0,
    };
    let hint = if at.is_none() {
        "; a deleted node can be found with --at <revision>"
    } else {
        ""
    };
    let node = graph
        .get(id)
        .with_context(|| format!("no node with id {id}{hint}"))?;

    let start = revision.as_ref().map_or("HEAD", |r| r.oid.as_str());
    let commits = git::file_history(root, start, &node.path, limit + 1)?;
    let truncated = commits.len() > limit;

    // The node as it was after each commit. The extra commit is only read to
    // compare against, never shown.
    let versions: Vec<Option<Node>> = commits
        .iter()
        .map(|commit| {
            let bytes = git::file_at(root, &commit.oid, &commit.path)?;
            Ok(bytes
                .and_then(|b| String::from_utf8(b).ok())
                .and_then(|text| parse_file_text(&commit.path, &text).0))
        })
        .collect::<Result<_>>()?;

    let mut entries: Vec<Entry> = Vec::new();
    if at.is_none() {
        let committed = versions.first().and_then(Option::as_ref);
        let event = Event::between(committed, Some(node));
        if !matches!(&event, Event::Changed(changes) if changes.is_empty()) {
            entries.push(Entry {
                commit: None,
                event,
            });
        }
    }
    for (i, commit) in commits.iter().take(limit).enumerate() {
        let new = versions[i].as_ref();
        let old = versions.get(i + 1).and_then(Option::as_ref);
        entries.push(Entry {
            commit: Some(commit.clone()),
            event: Event::between(old, new),
        });
    }

    if json_output {
        let entries: Vec<_> = entries
            .iter()
            .map(|e| {
                json!({
                    "commit": e.commit.as_ref().map(|c| json!({
                        "commit": c.oid, "short": c.short, "author": c.author,
                        "date": c.date, "subject": c.subject, "path": c.path,
                    })),
                    "event": if e.commit.is_none() { "uncommitted" } else { e.event.name() },
                    "changes": e.event.changes(),
                })
            })
            .collect();
        let out = json!({
            "schema_version": OUTPUT_SCHEMA_VERSION,
            "id": node.id,
            "path": node.path,
            "at": revision.as_ref().map(revision_json),
            "truncated": truncated,
            "entries": entries,
        });
        outln!("{}", serde_json::to_string_pretty(&out)?);
        return Ok(());
    }

    outln!("{}  {}", node.id, node.path.display());
    if entries.is_empty() {
        outln!("  no commits yet for this file");
    }
    for entry in &entries {
        match &entry.commit {
            Some(c) => outln!("  {}  {}  {}  {}", c.short, c.date, c.author, c.subject),
            None => outln!("  uncommitted"),
        }
        let what = match (&entry.commit, &entry.event) {
            (None, Event::Created) => "created (not committed)".to_string(),
            _ => entry.event.describe(),
        };
        outln!("      {what}");
    }
    if truncated {
        outln!("  more history exists; show it with --limit");
    }
    Ok(())
}

pub fn diff_command(
    root: &Path,
    from: &str,
    to: Option<&str>,
    json_output: bool,
    cache: bool,
) -> Result<()> {
    config::load(root)?;
    let from_rev = git::resolve_rev(root, from)?;
    let old = git::graph_at(root, &from_rev)?;
    let to_rev = to.map(|spec| git::resolve_rev(root, spec)).transpose()?;
    let new = match &to_rev {
        Some(rev) => git::graph_at(root, rev)?,
        None => index::load(root, cache).0,
    };
    let diff = diff_graphs(&old, &new);

    if json_output {
        let out = json!({
            "schema_version": OUTPUT_SCHEMA_VERSION,
            "from": revision_json(&from_rev),
            "to": to_rev.as_ref().map(revision_json),
            "added": diff.added,
            "removed": diff.removed,
            "changed": diff.changed,
            "new_findings": diff.new_findings,
            "fixed_findings": diff.fixed_findings,
        });
        outln!("{}", serde_json::to_string_pretty(&out)?);
        return Ok(());
    }

    let target = to_rev
        .as_ref()
        .map_or("the working tree".to_string(), |r| r.to_string());
    outln!("from {from_rev} to {target}");
    for n in &diff.added {
        outln!("+ {}  {}", n.id, n.path.display());
    }
    for n in &diff.removed {
        outln!("- {}  {}", n.id, n.path.display());
    }
    for n in &diff.changed {
        outln!("~ {}  {}", n.id, n.path.display());
        for change in &n.changes {
            outln!("    {}", change.describe());
        }
    }
    if diff.added.is_empty() && diff.removed.is_empty() && diff.changed.is_empty() {
        outln!("no changes to any node");
    }
    outln!(
        "findings: {} new, {} fixed",
        diff.new_findings.len(),
        diff.fixed_findings.len()
    );
    for f in &diff.new_findings {
        outln!("  new {}", crate::finding_line(f));
    }
    for f in &diff.fixed_findings {
        outln!("  fixed {}", crate::finding_line(f));
    }
    Ok(())
}

/// `oxispec tag`: with a name, creates the tag `specs-<name>`. Without one,
/// lists the spec tags.
pub fn tag_command(
    root: &Path,
    name: Option<&str>,
    at: Option<&str>,
    message: Option<&str>,
    json_output: bool,
) -> Result<()> {
    config::load(root)?;
    git::require_repository(root, "tag")?;
    let Some(name) = name else {
        if at.is_some() || message.is_some() {
            anyhow::bail!("--at and --message are for creating a tag, so give it a name");
        }
        let tags = git::list_tags(root)?;
        if json_output {
            let list: Vec<_> = tags
                .iter()
                .map(|t| json!({"name": t.name, "tag": t.tag, "commit": t.commit, "date": t.date, "message": t.message}))
                .collect();
            let out = json!({"schema_version": OUTPUT_SCHEMA_VERSION, "tags": list});
            outln!("{}", serde_json::to_string_pretty(&out)?);
        } else if tags.is_empty() {
            outln!("no spec tags yet; create one with `oxispec tag <name>`");
        } else {
            for t in &tags {
                outln!("{:<16} {}  {}  {}", t.name, t.commit, t.date, t.message);
            }
        }
        return Ok(());
    };

    let revision = git::resolve_rev(root, at.unwrap_or("HEAD"))?;
    let default_message = format!("OxiSpec tag {name}");
    let tag = git::create_tag(
        root,
        name,
        &revision.oid,
        message.unwrap_or(&default_message),
    )?;
    let loose = if at.is_none() {
        git::uncommitted_spec_files(root)?
    } else {
        0
    };
    if json_output {
        let out = json!({
            "schema_version": OUTPUT_SCHEMA_VERSION,
            "tag": tag,
            "name": name,
            "commit": revision.oid,
            "uncommitted_spec_files": loose,
        });
        outln!("{}", serde_json::to_string_pretty(&out)?);
    } else {
        outln!("tagged {revision} as {tag}; use it with --at {name}");
        if loose > 0 {
            outln!(
                "note: {loose} spec file(s) have changes that are not committed, so the tag does not include them"
            );
        }
    }
    Ok(())
}
