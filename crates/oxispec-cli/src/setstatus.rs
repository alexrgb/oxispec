//! `oxispec set-status`: the status of requirements, one at a time or a functionality at once.

use std::fs;
use std::path::Path;

use anyhow::{Context, Result, bail};
use oxispec_core::edit::set_fields;
use oxispec_core::node::Node;
use oxispec_core::{OUTPUT_SCHEMA_VERSION, config, formats, index};
use serde_json::json;

use crate::readdomain;

/// What `set-status` was asked to do.
pub struct Request<'a> {
    pub status: &'a str,
    pub ids: &'a [String],
    pub domain: Option<&'a str>,
    pub from: Option<&'a str>,
    pub dry_run: bool,
}

// spec: REQ-IMPORT-0005
pub fn set_status(root: &Path, req: &Request, json_output: bool, cache: bool) -> Result<()> {
    let Request {
        status,
        ids,
        domain,
        from,
        dry_run,
    } = *req;
    let cfg = config::load(root)?;
    if ids.is_empty() && domain.is_none() {
        bail!("say which requirements: their IDs, or --domain NAME (see `oxispec read`)");
    }
    let (profiles, _) = formats::available(&cfg);
    let name = cfg
        .formats
        .get("requirement")
        .map_or("shall", String::as_str);
    let profile = profiles
        .iter()
        .find(|p| p.name == name && p.ty == "requirement")
        .with_context(|| format!("no requirement profile named `{name}`"))?;
    if !profile.statuses.is_empty() && !profile.statuses.iter().any(|s| s == status) {
        bail!(
            "the status `{status}` is not one `{}` allows ({})",
            profile.name,
            profile.statuses.join(", ")
        );
    }
    let graph = index::load(root, cache).0;
    let mut chosen: Vec<&Node> = Vec::new();
    for id in ids {
        let node = graph
            .get(id)
            .with_context(|| format!("no node with id {id}"))?;
        if node.ty != "requirement" {
            bail!(
                "{} is a {}, not a requirement; change sets, debt and risks have their own `status` commands",
                node.id,
                node.ty
            );
        }
        chosen.push(node);
    }
    if let Some(asked) = domain {
        let found = readdomain::functionality(&graph, asked);
        if found.is_empty() {
            let known = readdomain::names(&graph);
            let known: Vec<&str> = known.iter().map(String::as_str).collect();
            bail!("{}", readdomain::none_found(asked, &known));
        }
        chosen.extend(found);
    }
    chosen.sort_by(|a, b| a.id.cmp(&b.id));
    chosen.dedup_by(|a, b| a.id == b.id);
    let mut rows = Vec::new();
    let (mut changed, mut same, mut left) = (0, 0, 0);
    for node in chosen {
        let was = node.status.clone().unwrap_or_default();
        let outcome = if from.is_some_and(|f| f != was) {
            left += 1;
            "left"
        } else if was == status {
            same += 1;
            "same"
        } else {
            changed += 1;
            if !dry_run {
                let path = root.join(&node.path);
                let text = fs::read_to_string(&path)
                    .with_context(|| format!("cannot read {}", path.display()))?;
                let updated = set_fields(&text, &[("status", Some(status))])
                    .with_context(|| format!("cannot edit {}", node.path.display()))?;
                fs::write(&path, updated)
                    .with_context(|| format!("cannot write {}", path.display()))?;
            }
            "changed"
        };
        rows.push((node.id.clone(), was, outcome));
    }
    if json_output {
        let list: Vec<_> = rows
            .iter()
            .map(|(id, was, outcome)| json!({"id": id, "from": was, "to": status, "result": outcome}))
            .collect();
        let out = json!({
            "schema_version": OUTPUT_SCHEMA_VERSION,
            "dry_run": dry_run,
            "status": status,
            "changed": changed,
            "unchanged": same,
            "left": left,
            "requirements": list,
        });
        outln!("{}", serde_json::to_string_pretty(&out)?);
        return Ok(());
    }
    for (id, was, outcome) in &rows {
        if *outcome == "changed" {
            let was = if was.is_empty() { "no status" } else { was };
            outln!("{id}: {was} -> {status}");
        }
    }
    let verb = if dry_run { "would change" } else { "changed" };
    let mut line = format!("{verb} {changed} requirement(s) to {status}");
    if same > 0 {
        line.push_str(&format!(", {same} already {status}"));
    }
    if left > 0 {
        line.push_str(&format!(
            ", {left} left as they are (not from {})",
            from.unwrap_or("")
        ));
    }
    outln!("{line}");
    if dry_run {
        outln!("nothing was written");
    }
    Ok(())
}
