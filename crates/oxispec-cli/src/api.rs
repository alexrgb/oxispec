//! `oxispec api show`: everything the project says about an operation.

use std::collections::BTreeSet;
use std::path::Path;

use anyhow::{Context, Result, bail};
use oxispec_core::api::{self, Card};
use oxispec_core::trace::{coverage, scan_tree_with};
use oxispec_core::{OUTPUT_SCHEMA_VERSION, config, results};
use serde_json::json;

use crate::{load_graph, require_module};

/// How a scenario stands: from the tests and the imported results, when the
/// `tests` module is on.
fn standing(root: &Path, cache: bool) -> Result<Option<(BTreeSet<String>, BTreeSet<String>)>> {
    let cfg = config::load(root)?;
    if !cfg.has("tests") {
        return Ok(None);
    }
    let scan = scan_tree_with(root, &cfg.trace)?;
    let graph = load_graph(root, cache);
    let unverified = coverage(&graph, &scan.annotations, None)
        .unverified
        .into_iter()
        .collect();
    let failing = match results::load(root)? {
        Some(found) => results::scenario_status(&graph, &scan.annotations, &found)
            .into_iter()
            .filter(|(_, s)| *s == results::Status::Failing)
            .map(|(s, _)| s)
            .collect(),
        None => BTreeSet::new(),
    };
    Ok(Some((unverified, failing)))
}

fn state(id: &str, known: &Option<(BTreeSet<String>, BTreeSet<String>)>) -> &'static str {
    match known {
        None => "unknown (module tests is off)",
        Some((_, failing)) if failing.contains(id) => "failing",
        Some((unverified, _)) if unverified.contains(id) => "no test",
        Some(_) => "verified",
    }
}

fn card_json(c: &Card, known: &Option<(BTreeSet<String>, BTreeSet<String>)>) -> serde_json::Value {
    let o = c.operation;
    json!({
        "call": o.call(),
        "tag": o.tag,
        "summary": o.summary,
        "requirement": o.node,
        "status": c.status,
        "responses": o.responses,
        "scenarios": c.scenarios.iter().map(|s| json!({"id": s, "state": state(s, known)})).collect::<Vec<_>>(),
        "prds": c.prds,
        "decisions": c.decisions,
        "debt": c.debt,
        "risks": c.risks.iter().map(|(id, level)| json!({"id": id, "level": level})).collect::<Vec<_>>(),
        "test_plans": c.plans,
    })
}

pub fn show(root: &Path, target: &str, json_output: bool, cache: bool) -> Result<()> {
    let cfg = config::load(root)?;
    require_module(&cfg, "specs")?;
    let graph = load_graph(root, cache);
    let ops = api::operations(&graph, root)?;
    let chosen = api::select(&ops, target);
    if chosen.is_empty() {
        let known: Vec<String> = ops.iter().map(api::Operation::call).collect();
        bail!(
            "no operation matches `{target}`; the requirements state {}",
            if known.is_empty() {
                "none (a requirement block titled `GET /path` is an operation)".to_string()
            } else {
                known.join(", ")
            }
        );
    }
    let known = standing(root, cache)?;
    let cards: Vec<Card> = chosen.iter().map(|o| api::card(&graph, o)).collect();
    if json_output {
        outln!(
            "{}",
            serde_json::to_string_pretty(&json!({
                "schema_version": OUTPUT_SCHEMA_VERSION,
                "operations": cards.iter().map(|c| card_json(c, &known)).collect::<Vec<_>>(),
            }))?
        );
        return Ok(());
    }
    for c in &cards {
        let o = c.operation;
        outln!("{}  [{}]", o.call(), o.tag);
        if !o.summary.is_empty() {
            outln!("    {}", o.summary);
        }
        outln!(
            "    requirement {} ({})",
            o.node,
            c.status.as_deref().unwrap_or("no status")
        );
        let list = |label: &str, items: &[String]| {
            if !items.is_empty() {
                outln!("    {label}: {}", items.join(", "));
            }
        };
        list("PRD", &c.prds);
        list("decisions", &c.decisions);
        list("tech debt", &c.debt);
        let risks: Vec<String> = c
            .risks
            .iter()
            .map(|(id, level)| format!("{id} ({})", level.unwrap_or("no level")))
            .collect();
        list("risks", &risks);
        list("test plans", &c.plans);
        for (code, what) in &o.responses {
            outln!(
                "    responds {code}{}",
                if what.is_empty() {
                    String::new()
                } else {
                    format!(": {what}")
                }
            );
        }
        for s in &c.scenarios {
            outln!("    {s}: {}", state(s, &known));
        }
    }
    Ok(())
}

/// Embeds text with a service that has the OpenAI-compatible embeddings API.
struct HttpEmbedder {
    url: String,
    model: String,
    key: Option<String>,
}

impl oxispec_core::apiindex::Embedder for HttpEmbedder {
    fn id(&self) -> String {
        format!("http {} {}", self.url, self.model)
    }

    fn embed(&self, texts: &[String]) -> std::result::Result<Vec<Vec<f32>>, String> {
        let mut headers = Vec::new();
        if let Some(key) = &self.key {
            headers.push(format!("Authorization: Bearer {key}"));
        }
        let mut out = Vec::new();
        for batch in texts.chunks(16) {
            let body = serde_json::json!({"model": self.model, "input": batch}).to_string();
            let answer = crate::fetch::post_json(&self.url, &headers, &body)
                .map_err(|e| format!("the embedder {} failed: {e:#}", self.url))?;
            let value: serde_json::Value = serde_json::from_str(&answer)
                .map_err(|e| format!("the embedder {} did not answer with JSON: {e}", self.url))?;
            let mut rows: Vec<(u64, Vec<f32>)> = value["data"]
                .as_array()
                .ok_or_else(|| format!("the answer of {} has no `data` list", self.url))?
                .iter()
                .enumerate()
                .map(|(i, row)| {
                    let v: Vec<f32> = row["embedding"]
                        .as_array()
                        .map(|a| {
                            a.iter()
                                .filter_map(|x| x.as_f64())
                                .map(|x| x as f32)
                                .collect()
                        })
                        .unwrap_or_default();
                    (row["index"].as_u64().unwrap_or(i as u64), v)
                })
                .collect();
            rows.sort_by_key(|(i, _)| *i);
            if rows.len() != batch.len() || rows.iter().any(|(_, v)| v.is_empty()) {
                return Err(format!(
                    "the embedder {} gave {} vectors for {} texts",
                    self.url,
                    rows.len(),
                    batch.len()
                ));
            }
            for (_, mut v) in rows {
                let norm = v.iter().map(|x| x * x).sum::<f32>().sqrt();
                if norm > 0.0 {
                    v.iter_mut().for_each(|x| *x /= norm);
                }
                out.push(v);
            }
        }
        Ok(out)
    }
}

fn embedder(root: &Path) -> Result<Box<dyn oxispec_core::apiindex::Embedder>> {
    let cfg = config::load(root)?;
    Ok(match cfg.api_index.embedder {
        None => Box::new(oxispec_core::apiindex::HashEmbedder),
        Some(e) => {
            let key = match &e.api_key_env {
                Some(name) => Some(
                    std::env::var(name)
                        .with_context(|| format!("the environment variable {name} is not set"))?,
                ),
                None => None,
            };
            Box::new(HttpEmbedder {
                url: e.url,
                model: e.model,
                key,
            })
        }
    })
}

// spec: REQ-APIINDEX-0004
/// Builds or updates the index of the operations.
pub fn index(root: &Path, openapi: Option<&str>, json_output: bool, cache: bool) -> Result<()> {
    use oxispec_core::apiindex;
    let cfg = config::load(root)?;
    require_module(&cfg, "specs")?;
    let graph = load_graph(root, cache);
    let ops = api::operations(&graph, root)?;
    if ops.is_empty() {
        bail!(
            "no requirement is titled as a call; a block titled `GET /orders` is an operation (see the API guide)"
        );
    }
    let doc = match openapi {
        None => None,
        Some(src) if src.starts_with("http://") || src.starts_with("https://") => {
            Some(api::parse(&crate::fetch::get(src)?, src)?)
        }
        Some(path) => Some(api::load(Path::new(path))?),
    };
    let embedder = embedder(root)?;
    let chunks = apiindex::chunks(&graph, &ops, doc.as_ref());
    let mut store = apiindex::load(root).unwrap_or_else(|| apiindex::Store::new(&embedder.id()));
    let stats =
        apiindex::sync(&mut store, chunks, embedder.as_ref()).map_err(anyhow::Error::msg)?;
    apiindex::save(root, &store)?;
    if json_output {
        outln!(
            "{}",
            serde_json::to_string_pretty(&json!({
                "schema_version": OUTPUT_SCHEMA_VERSION,
                "embedder": store.embedder,
                "operations": store.entries.len(),
                "added": stats.added,
                "updated": stats.updated,
                "removed": stats.removed,
                "unchanged": stats.unchanged,
                "path": apiindex::INDEX_PATH,
            }))?
        );
        return Ok(());
    }
    outln!(
        "{} operation(s) in {} ({}): {} added, {} updated, {} removed, {} unchanged",
        store.entries.len(),
        apiindex::INDEX_PATH,
        store.embedder,
        stats.added,
        stats.updated,
        stats.removed,
        stats.unchanged
    );
    Ok(())
}

// spec: REQ-APIINDEX-0004
/// Answers a query with the nearest operations.
pub fn search(root: &Path, query: &str, top: usize, json_output: bool) -> Result<()> {
    use oxispec_core::apiindex;
    let cfg = config::load(root)?;
    require_module(&cfg, "specs")?;
    let Some(store) = apiindex::load(root) else {
        bail!("there is no index yet; build it with `oxispec api index`");
    };
    let embedder = embedder(root)?;
    if store.embedder != embedder.id() {
        bail!(
            "the index was made with `{}` and the config now says `{}`; run `oxispec api index` again",
            store.embedder,
            embedder.id()
        );
    }
    let vector = embedder
        .embed(&[query.to_string()])
        .map_err(anyhow::Error::msg)?
        .remove(0);
    let hits = apiindex::search(&store, &vector, top);
    if json_output {
        let rows: Vec<_> = hits
            .iter()
            .map(|h| {
                json!({
                    "call": h.entry.call,
                    "requirement": h.entry.node,
                    "tag": h.entry.tag,
                    "status": h.entry.status,
                    "score": h.score,
                    "text": h.entry.text,
                })
            })
            .collect();
        outln!(
            "{}",
            serde_json::to_string_pretty(&json!({
                "schema_version": OUTPUT_SCHEMA_VERSION,
                "query": query,
                "embedder": store.embedder,
                "hits": rows,
            }))?
        );
        return Ok(());
    }
    if hits.is_empty() {
        outln!("nothing in the index");
    }
    for h in &hits {
        outln!(
            "{:.2}  {}  [{}]  {}",
            h.score,
            h.entry.call,
            h.entry.tag,
            h.entry.node
        );
    }
    Ok(())
}
