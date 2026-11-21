//! `oxispec search`: full-text search over the nodes.

use std::path::Path;

use anyhow::{Result, bail};
use oxispec_core::search::{SearchQuery, search};
use oxispec_core::{OUTPUT_SCHEMA_VERSION, config, index};
use serde_json::json;

pub struct SearchOptions {
    pub all_products: bool,
    pub query: String,
    pub ty: Vec<String>,
    pub status: Option<String>,
    pub limit: usize,
    pub rebuild: bool,
    pub json: bool,
    pub cache: bool,
}

// spec: REQ-PROJECT-0009
pub fn search_command(root: &Path, options: &SearchOptions) -> Result<()> {
    config::load(root)?;
    if options.query.trim().is_empty() {
        bail!("give some words to search for");
    }
    let projects = if options.all_products {
        crate::scope::projects(root)
    } else {
        vec![(".".to_string(), root.to_path_buf())]
    };
    let mut hits = Vec::new();
    let mut top_stats = None;
    for (product, dir) in &projects {
        let graph = index::load(dir, options.cache).0;
        let query = SearchQuery {
            text: options.query.clone(),
            types: options.ty.clone(),
            status: options.status.clone(),
            limit: options.limit,
        };
        let (found, stats) = search(dir, &graph, options.cache, options.rebuild, &query)?;
        if top_stats.is_none() {
            top_stats = Some(stats);
        }
        hits.extend(found.into_iter().map(|h| (product.clone(), h)));
    }
    let stats = top_stats.unwrap_or_default();

    if options.json {
        let hits: Vec<_> = hits
            .iter()
            .map(|(product, hit)| {
                let mut value = serde_json::to_value(hit).unwrap_or_default();
                if options.all_products {
                    value["product"] = product.clone().into();
                }
                value
            })
            .collect();
        let out = json!({
            "schema_version": OUTPUT_SCHEMA_VERSION,
            "query": options.query,
            "index": stats,
            "hits": hits,
        });
        outln!("{}", serde_json::to_string_pretty(&out)?);
        return Ok(());
    }
    if stats.changed() > 0 {
        eprintln!(
            "updated the search index ({} added, {} changed, {} removed)",
            stats.added, stats.updated, stats.removed
        );
    }
    if hits.is_empty() {
        outln!("no matches");
    }
    for (product, hit) in &hits {
        let name = if options.all_products && product != "." {
            format!("{product}:{}", hit.id)
        } else {
            hit.id.clone()
        };
        outln!(
            "{}  {}  {}  {}",
            name,
            hit.ty,
            hit.status.as_deref().unwrap_or("-"),
            hit.title.as_deref().unwrap_or("")
        );
        outln!("  {}", hit.path.display());
        if let Some(snippet) = &hit.snippet {
            outln!("  {snippet}");
        }
    }
    Ok(())
}
