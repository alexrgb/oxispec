//! `oxispec read`: the whole specification of one functionality on one page. A functionality is the
//! folder of its requirements (`authentication`) or the domain of their IDs (`AUTH`).

use std::collections::BTreeMap;
use std::path::Path;

use anyhow::{Context, Result, bail};
use oxispec_core::node::Node;
use oxispec_core::{Graph, OUTPUT_SCHEMA_VERSION, config, id, index};
use serde_json::json;

/// The domain of a requirement ID, with its hyphens: `REQ-ACCOUNT-AND-PROFILE-0001` gives
/// `ACCOUNT-AND-PROFILE`.
fn domain_of(node_id: &str) -> Option<String> {
    let parts: Vec<&str> = node_id.split('-').collect();
    (parts.len() >= 3).then(|| parts[1..parts.len() - 1].join("-"))
}

/// Capitals without hyphens, so `account-and-profile`, `ACCOUNT_AND_PROFILE` and `accountandprofile`
/// are the same domain.
fn squashed(name: &str) -> String {
    name.to_ascii_uppercase().replace(['-', '_', ' '], "")
}

/// The folder a requirement is in under `requirements/`, such as `authentication`; none for a
/// requirement written straight in `requirements/`.
fn folder_of(node: &Node) -> Option<String> {
    let dir = node.path.parent()?;
    let name = dir.file_name()?.to_str()?;
    (name != "requirements").then(|| name.to_string())
}

/// What a functionality is called: its folder, or the domain of its IDs when it has no folder.
fn group_of(node: &Node) -> Option<String> {
    folder_of(node).or_else(|| domain_of(&node.id).map(|d| d.to_ascii_lowercase()))
}

/// The functionalities of the requirements: their name, the domain their IDs use, and how many
/// requirements each has.
fn groups(graph: &Graph) -> BTreeMap<String, (String, usize)> {
    let mut out: BTreeMap<String, (String, usize)> = BTreeMap::new();
    for n in graph.nodes.iter().filter(|n| n.ty == "requirement") {
        if let (Some(g), Some(d)) = (group_of(n), domain_of(&n.id)) {
            out.entry(g).or_insert((d, 0)).1 += 1;
        }
    }
    out
}

/// The order of IDs for reading: by domain, then by number as a number (`REQ-AUTH-2` before
/// `REQ-AUTH-10`), then by what is left.
fn id_order(node_id: &str) -> (String, u64, String) {
    match node_id.rsplit_once('-') {
        Some((head, tail)) => match tail.parse::<u64>() {
            Ok(n) => (head.to_string(), n, String::new()),
            Err(_) => (head.to_string(), 0, tail.to_string()),
        },
        None => (node_id.to_string(), 0, String::new()),
    }
}

/// The text of a node without its front matter.
fn body_of(root: &Path, node: &Node) -> Result<String> {
    let raw = std::fs::read_to_string(root.join(&node.path))
        .with_context(|| format!("cannot read {}", node.path.display()))?
        .replace("\r\n", "\n");
    let body = oxispec_core::parse::frontmatter_span(&raw)
        .map_or(raw.as_str(), |span| &raw[span.body_start..]);
    Ok(body.trim().to_string())
}

// spec: REQ-SEARCH-0004
pub fn read(
    root: &Path,
    domain: Option<&str>,
    status: Option<&str>,
    json_output: bool,
    cache: bool,
) -> Result<()> {
    config::load(root)?;
    let graph = index::load(root, cache).0;
    let all = groups(&graph);
    let Some(asked) = domain else {
        return list_domains(&all, json_output);
    };
    let key = squashed(asked);
    let mut nodes: Vec<&Node> = graph
        .nodes
        .iter()
        .filter(|n| n.ty == "requirement")
        .filter(|n| {
            group_of(n).is_some_and(|g| squashed(&g) == key)
                || domain_of(&n.id).is_some_and(|d| squashed(&d) == key)
        })
        .collect();
    if nodes.is_empty() {
        let known: Vec<&str> = all.keys().map(String::as_str).collect();
        bail!(
            "no requirements for `{asked}`{}",
            if known.is_empty() {
                String::new()
            } else {
                format!("; the functionalities are {}", known.join(", "))
            }
        );
    }
    let name = asked.to_string();
    nodes.retain(|n| status.is_none_or(|s| n.status.as_deref() == Some(s)));
    nodes.sort_by_key(|n| id_order(&n.id));
    // Change sets that are not finished and change something of this domain.
    let ids: Vec<&str> = nodes.iter().map(|n| n.id.as_str()).collect();
    let open: Vec<&Node> = graph
        .nodes
        .iter()
        .filter(|c| c.ty == "change")
        .filter(|c| !matches!(c.status.as_deref(), Some("merged" | "reverted")))
        .filter(|c| {
            c.links.get("changes").is_some_and(|targets| {
                targets.iter().any(|t| {
                    graph
                        .get(id::base_of(t))
                        .is_some_and(|n| ids.contains(&n.id.as_str()))
                })
            })
        })
        .collect();
    let mut texts = Vec::new();
    for n in &nodes {
        texts.push(body_of(root, n)?);
    }
    if json_output {
        let requirements: Vec<_> = nodes
            .iter()
            .zip(&texts)
            .map(|(n, text)| {
                json!({
                    "id": n.id,
                    "status": n.status,
                    "title": n.title,
                    "path": n.path,
                    "aliases": n.aliases,
                    "scenarios": n.requirements.iter().flat_map(|r| &r.scenarios)
                        .filter_map(|s| s.anchor.clone()).collect::<Vec<_>>(),
                    "text": text,
                })
            })
            .collect();
        let open: Vec<_> = open
            .iter()
            .map(|c| {
                let (done, total) = c.progress();
                json!({"id": c.id, "status": c.status, "title": c.title,
                       "tasks": {"done": done, "total": total}})
            })
            .collect();
        let out = json!({
            "schema_version": OUTPUT_SCHEMA_VERSION,
            "name": name,
            "requirements": requirements,
            "open_changes": open,
        });
        outln!("{}", serde_json::to_string_pretty(&out)?);
        return Ok(());
    }
    let scenarios: usize = nodes
        .iter()
        .map(|n| {
            n.requirements
                .iter()
                .map(|r| r.scenarios.len())
                .sum::<usize>()
        })
        .sum();
    outln!(
        "# {name}: {} requirement(s), {scenarios} scenario(s)",
        nodes.len()
    );
    for (n, text) in nodes.iter().zip(&texts) {
        outln!(
            "\n---\n\n## {} [{}]  {}\n\n{text}",
            n.id,
            n.status.as_deref().unwrap_or("no status"),
            n.path.display()
        );
    }
    if !open.is_empty() {
        outln!("\n---\n\n## Open change sets");
        for c in open {
            let (done, total) = c.progress();
            let tasks = if total == 0 {
                String::new()
            } else {
                format!("  [{done}/{total} tasks]")
            };
            outln!(
                "- {} [{}] {}{tasks}",
                c.id,
                c.status.as_deref().unwrap_or("no status"),
                c.title.as_deref().unwrap_or("")
            );
        }
    }
    Ok(())
}

fn list_domains(all: &BTreeMap<String, (String, usize)>, json_output: bool) -> Result<()> {
    if json_output {
        let list: Vec<_> = all
            .iter()
            .map(|(g, (d, n))| json!({"name": g, "domain": d, "requirements": n}))
            .collect();
        let out = json!({"schema_version": OUTPUT_SCHEMA_VERSION, "functionalities": list});
        outln!("{}", serde_json::to_string_pretty(&out)?);
        return Ok(());
    }
    if all.is_empty() {
        outln!("no requirements yet");
    }
    for (g, (d, n)) in all {
        outln!("{g:<32} REQ-{d:<24} {n} requirement(s)");
    }
    Ok(())
}
