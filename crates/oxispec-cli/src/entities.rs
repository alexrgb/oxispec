//! `oxispec entities`: the things the specs are about, and what depends on each.

use std::path::Path;

use anyhow::{Result, bail};
use oxispec_core::entities::{self, affected};
use oxispec_core::{OUTPUT_SCHEMA_VERSION, config};
use serde_json::json;

use crate::{load_graph, require_module};

// spec: REQ-ENTITY-0004
pub fn run(root: &Path, id: Option<&str>, depth: u32, json: bool, cache: bool) -> Result<()> {
    let cfg = config::load(root)?;
    require_module(&cfg, "consistency")?;
    let graph = load_graph(root, cache);
    let (all, _) = entities::read(&graph);
    let chosen: Vec<&entities::Entity> = match id {
        None => all.iter().collect(),
        Some(id) => match all.iter().find(|e| e.id.eq_ignore_ascii_case(id)) {
            Some(e) => vec![e],
            None => bail!("no entity {id}; `oxispec entities` lists them"),
        },
    };
    // A list gives the number of dependents, one entity gives them all.
    let rows: Vec<_> = chosen
        .iter()
        .map(|e| (*e, affected(&graph, &e.id, depth)))
        .collect();
    if json {
        let out: Vec<_> = rows
            .iter()
            .map(|(e, a)| {
                json!({
                    "id": e.id,
                    "name": e.name,
                    "title": e.title,
                    "path": e.path,
                    "attributes": e.attributes,
                    "affected": a,
                })
            })
            .collect();
        outln!(
            "{}",
            serde_json::to_string_pretty(&json!({
                "schema_version": OUTPUT_SCHEMA_VERSION,
                "depth": depth,
                "entities": out,
            }))?
        );
        return Ok(());
    }
    if rows.is_empty() {
        outln!("no entities; `oxispec new entity \"Order\"` makes one");
    }
    for (e, a) in &rows {
        let title = e.title.as_deref().unwrap_or("");
        outln!("{}  {}  {title}", e.id, e.name);
        for (name, attr) in &e.attributes {
            if attr.kind == "enum" {
                outln!("    {}.{name}  enum: {}", e.name, attr.values.join(", "));
            } else {
                outln!("    {}.{name}  {}", e.name, attr.kind);
            }
        }
        if id.is_some() {
            if a.is_empty() {
                outln!("  nothing depends on it yet");
            }
            for x in a {
                let title = x.title.as_deref().unwrap_or("");
                outln!(
                    "  {} hop{}  {}  {}  ({})  {title}",
                    x.distance,
                    if x.distance == 1 { " " } else { "s" },
                    x.id,
                    x.ty,
                    x.via
                );
            }
        } else {
            outln!("  {} depend on it", a.len());
        }
    }
    Ok(())
}
