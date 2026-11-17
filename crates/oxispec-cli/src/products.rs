//! `oxispec products`: the projects of a monorepo that sit below this one.

use std::path::Path;

use anyhow::Result;
use oxispec_core::{OUTPUT_SCHEMA_VERSION, config, products};
use serde_json::json;

// spec: REQ-PROJECT-0007
pub fn list(root: &Path, json: bool, cache: bool) -> Result<()> {
    config::load(root)?;
    let mut rows = Vec::new();
    for rel in products::find(root) {
        let dir = root.join(&rel);
        let cfg = config::load(&dir)?;
        let nodes = oxispec_core::index::load(&dir, cache).0.nodes.len();
        rows.push((products::slash(&rel), nodes, cfg.modules));
    }
    if json {
        let items: Vec<_> = rows
            .iter()
            .map(|(p, n, m)| json!({"path": p, "nodes": n, "modules": m}))
            .collect();
        outln!(
            "{}",
            serde_json::to_string_pretty(
                &json!({"schema_version": OUTPUT_SCHEMA_VERSION, "products": items})
            )?
        );
        return Ok(());
    }
    if rows.is_empty() {
        outln!("no products: no folder below this project has a project of its own");
        return Ok(());
    }
    for (path, nodes, modules) in &rows {
        outln!("{path:<32} {nodes} node(s)  {}", modules.join(", "));
    }
    Ok(())
}
