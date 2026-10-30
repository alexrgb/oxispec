//! `oxispec context`: the text of some nodes and what is near them, sized for an agent.

use std::fs;
use std::path::Path;

use anyhow::{Context, Result, bail};
use oxispec_core::context::{self, Pack};
use oxispec_core::{Graph, Node, OUTPUT_SCHEMA_VERSION, config, index};

use crate::git;

/// The pack for these nodes, from the working tree or from a git revision.
pub fn build(
    root: &Path,
    ids: &[String],
    depth: u32,
    budget: usize,
    at: Option<&str>,
    cache: bool,
) -> Result<Pack> {
    config::load(root)?;
    if ids.is_empty() {
        bail!("name at least one node");
    }
    let revision = at.map(|spec| git::resolve_rev(root, spec)).transpose()?;
    let graph: Graph = match &revision {
        Some(rev) => git::graph_at(root, rev)?,
        None => index::load(root, cache).0,
    };
    for id in ids {
        if graph.get(id).is_none() {
            bail!("no node with id {id}");
        }
    }
    let text_of = |node: &Node| -> Option<String> {
        match &revision {
            Some(rev) => git::file_at(root, &rev.oid, &node.path)
                .ok()
                .flatten()
                .map(|b| String::from_utf8_lossy(&b).into_owned()),
            None => fs::read_to_string(root.join(&node.path)).ok(),
        }
    };
    Ok(context::pack(&graph, ids, depth, budget, &text_of))
}

/// The pack as markdown, which is what an agent reads.
pub fn markdown(pack: &Pack) -> String {
    let mut out = format!(
        "# Context pack: {} of {} tokens used\n\n",
        pack.used, pack.budget
    );
    for item in &pack.items {
        let status = item.status.as_deref().unwrap_or("no status");
        let how = match &item.connection {
            Some(c) => format!("{} hops away: {c}", item.distance),
            None => "asked for".to_string(),
        };
        out.push_str(&format!(
            "## {} ({}, {status}): {how}\n\n{}\n\n",
            item.id,
            item.ty,
            item.text.trim_end()
        ));
    }
    if !pack.pointers.is_empty() {
        out.push_str("## Related, not included\n\nThese did not fit. Ask for them by ID.\n\n");
        for p in &pack.pointers {
            out.push_str(&format!(
                "- {} ({}): {}\n",
                p.id,
                p.ty,
                p.title.as_deref().unwrap_or("")
            ));
        }
        if pack.more > 0 {
            out.push_str(&format!("- and {} more\n", pack.more));
        }
    }
    out
}

pub fn context_command(
    root: &Path,
    ids: &[String],
    depth: u32,
    budget: usize,
    at: Option<&str>,
    json: bool,
    cache: bool,
) -> Result<()> {
    let pack = build(root, ids, depth, budget, at, cache).context("cannot build the pack")?;
    if json {
        let mut value = serde_json::to_value(&pack)?;
        value["schema_version"] = OUTPUT_SCHEMA_VERSION.into();
        println!("{}", serde_json::to_string_pretty(&value)?);
    } else {
        print!("{}", markdown(&pack));
    }
    Ok(())
}
