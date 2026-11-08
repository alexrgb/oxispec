//! `oxispec entities`: the things the specs are about, and what depends on each.

use std::path::Path;

use anyhow::{Result, bail};
use oxispec_core::entities::{self, affected};
use oxispec_core::trace::scan_tree_with;
use oxispec_core::{OUTPUT_SCHEMA_VERSION, config};
use serde_json::json;

use crate::{load_graph, require_module};

// spec: REQ-ENTITY-0004
// spec: REQ-ENTITY-0007
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
    // The code that says it is about each entity, when the project reads code at all.
    let annotations = if cfg.has("tests") {
        scan_tree_with(root, &cfg.trace)?.annotations
    } else {
        Vec::new()
    };
    // The nodes that are about an entity that changed since a person confirmed them.
    let stale: std::collections::BTreeSet<String> =
        oxispec_core::suspect::reviews(&graph, &oxispec_core::suspect::load(root)?)
            .into_iter()
            .filter(|r| r.state == oxispec_core::suspect::ReviewState::Suspect)
            .map(|r| format!("{} {}", r.node, r.entity))
            .collect();
    // A list gives the number of dependents, one entity gives them all.
    let rows: Vec<_> = chosen
        .iter()
        .map(|e| {
            let code = entities::code(&e.id, &annotations);
            let missing = entities::missing_in_code(e, &code);
            let mut dependents = affected(&graph, &e.id, depth);
            for d in &mut dependents {
                d.needs_review = stale.contains(&format!("{} {}", d.id, e.id));
            }
            (*e, dependents, code, missing)
        })
        .collect();
    if json {
        let out: Vec<_> = rows
            .iter()
            .map(|(e, a, code, missing)| {
                json!({
                    "id": e.id,
                    "name": e.name,
                    "title": e.title,
                    "path": e.path,
                    "attributes": e.attributes,
                    "affected": a,
                    "code": code,
                    "missing_in_code": missing,
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
    for (e, a, code, missing) in &rows {
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
            if code.is_empty() {
                outln!(
                    "  no code says it represents it (`// spec: {}` above a struct or class)",
                    e.id
                );
            }
            for c in code {
                let what = match (&c.kind, &c.symbol) {
                    (Some(kind), Some(symbol)) => format!("{symbol} ({kind})"),
                    _ => "(not bound to code)".to_string(),
                };
                let fields = if c.fields.is_empty() {
                    String::new()
                } else {
                    format!("  fields: {}", c.fields.join(", "))
                };
                outln!("  code  {}:{}  {what}{fields}", c.path.display(), c.line);
            }
            for name in missing {
                outln!("  ! no field for `{}.{name}` in that code", e.name);
            }
            if a.is_empty() {
                outln!("  nothing depends on it yet");
            }
            for x in a {
                let title = x.title.as_deref().unwrap_or("");
                outln!(
                    "  {} hop{}  {}  {}  ({})  {title}{}",
                    x.distance,
                    if x.distance == 1 { " " } else { "s" },
                    x.id,
                    x.ty,
                    x.via,
                    if x.needs_review { "  NEEDS REVIEW" } else { "" }
                );
            }
        } else {
            outln!("  {} depend on it, {} places in code", a.len(), code.len());
        }
    }
    Ok(())
}
