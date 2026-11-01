//! `oxispec search`: full-text search over the nodes.

use std::path::Path;

use anyhow::{Result, bail};
use oxispec_core::search::{SearchQuery, search};
use oxispec_core::{OUTPUT_SCHEMA_VERSION, config, index};
use serde_json::json;

pub struct SearchOptions {
    pub query: String,
    pub ty: Vec<String>,
    pub status: Option<String>,
    pub limit: usize,
    pub rebuild: bool,
    pub json: bool,
    pub cache: bool,
}

pub fn search_command(root: &Path, options: &SearchOptions) -> Result<()> {
    config::load(root)?;
    if options.query.trim().is_empty() {
        bail!("give some words to search for");
    }
    let graph = index::load(root, options.cache).0;
    let query = SearchQuery {
        text: options.query.clone(),
        types: options.ty.clone(),
        status: options.status.clone(),
        limit: options.limit,
    };
    let (hits, stats) = search(root, &graph, options.cache, options.rebuild, &query)?;

    if options.json {
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
    for hit in &hits {
        outln!(
            "{}  {}  {}  {}",
            hit.id,
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
