//! A search index over the operations of an API.
//!
//! One chunk of text is made for each operation: its call, its summary and
//! scenarios, the product requirements document, decisions, risks, tech debt
//! and test plans around it, and, when an OpenAPI file is given, what that
//! file says about it. Each chunk becomes a vector, and a query is answered by
//! the chunks whose vectors are nearest.
//!
//! How a text becomes a vector is the job of an [`Embedder`]. The one that
//! needs nothing is [`HashEmbedder`]: it counts words and word pairs into a
//! fixed number of buckets. It finds what shares words with the query, and
//! does not know that two different words mean the same. A project that wants
//! that points the index at a service that embeds text, in
//! `.oxispec/config.yaml`; the CLI talks to it.
//!
//! The index is a derived file. A chunk is embedded again only when its text
//! or the embedder changes.

use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::api::{self, Operation};
use crate::graph::Graph;

/// Where the index is kept, under the project root.
pub const INDEX_PATH: &str = ".oxispec/cache/api-index.json";

const FORMAT: u32 = 1;

/// The number of buckets of the built-in embedder.
pub const DIM: usize = 256;

/// Turns texts into vectors of one size.
pub trait Embedder {
    /// Names the embedder and its model. A different name makes every chunk
    /// be embedded again.
    fn id(&self) -> String;
    fn embed(&self, texts: &[String]) -> Result<Vec<Vec<f32>>, String>;
}

/// Counts words and word pairs into [`DIM`] buckets, with no model.
pub struct HashEmbedder;

impl Embedder for HashEmbedder {
    fn id(&self) -> String {
        format!("hash-{DIM}")
    }

    fn embed(&self, texts: &[String]) -> Result<Vec<Vec<f32>>, String> {
        Ok(texts.iter().map(|t| hash_embed(t)).collect())
    }
}

const STOP: &[&str] = &[
    "the", "a", "an", "of", "to", "in", "is", "it", "and", "or", "for", "on", "at", "by", "as",
    "be", "are", "was", "with", "that", "this", "from", "when", "then", "shall", "system",
];

/// Lower-case words of letters and digits, without stop words, each cut to a
/// crude stem so that `orders` and `order` meet.
pub fn tokens(text: &str) -> Vec<String> {
    text.split(|c: char| !c.is_alphanumeric())
        .filter(|w| w.len() > 1)
        .map(str::to_lowercase)
        .filter(|w| !STOP.contains(&w.as_str()))
        .map(|w| stem(&w))
        .collect()
}

fn stem(word: &str) -> String {
    for suffix in ["ing", "ed", "es", "s"] {
        if let Some(root) = word.strip_suffix(suffix)
            && root.len() >= 3
        {
            return root.to_string();
        }
    }
    word.to_string()
}

fn bucket(feature: &str) -> (usize, f32) {
    // FNV-1a: small, stable between runs and machines.
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in feature.bytes() {
        h ^= u64::from(b);
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    let sign = if (h >> 63) & 1 == 0 { 1.0 } else { -1.0 };
    ((h % DIM as u64) as usize, sign)
}

/// A unit vector for a text.
pub fn hash_embed(text: &str) -> Vec<f32> {
    let words = tokens(text);
    let mut counts: std::collections::BTreeMap<String, f32> = std::collections::BTreeMap::new();
    for w in &words {
        *counts.entry(w.clone()).or_insert(0.0) += 1.0;
    }
    for pair in words.windows(2) {
        *counts
            .entry(format!("{} {}", pair[0], pair[1]))
            .or_insert(0.0) += 0.5;
    }
    let mut v = vec![0.0f32; DIM];
    for (feature, n) in counts {
        let (i, sign) = bucket(&feature);
        v[i] += sign * (1.0 + n.ln_1p());
    }
    normalize(&mut v);
    v
}

fn normalize(v: &mut [f32]) {
    let norm = v.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm > 0.0 {
        for x in v {
            *x /= norm;
        }
    }
}

/// The cosine of the angle between two vectors of unit length.
pub fn cosine(a: &[f32], b: &[f32]) -> f32 {
    a.iter().zip(b).map(|(x, y)| x * y).sum()
}

/// The text made for one operation, before it is embedded.
#[derive(Debug, Clone, PartialEq)]
pub struct Chunk {
    pub key: String,
    pub text: String,
    pub node: String,
    pub call: String,
    pub tag: String,
    pub status: Option<String>,
}

fn title_of(graph: &Graph, id: &str) -> String {
    graph
        .get(id)
        .and_then(|n| n.title.clone())
        .map_or_else(|| id.to_string(), |t| format!("{id}: {t}"))
}

/// What an OpenAPI description says about one call, as plain text.
pub fn openapi_text(doc: &Value, method: &str, path: &str) -> Option<String> {
    let op = doc.pointer(&format!(
        "/paths/{}/{method}",
        path.replace('~', "~0").replace('/', "~1")
    ))?;
    let mut out = Vec::new();
    for key in ["summary", "description", "operationId"] {
        if let Some(t) = op.get(key).and_then(Value::as_str) {
            out.push(format!(
                "{key}: {}",
                t.split_whitespace().collect::<Vec<_>>().join(" ")
            ));
        }
    }
    if let Some(params) = op.get("parameters").and_then(Value::as_array) {
        for p in params {
            if let Some(name) = p.get("name").and_then(Value::as_str) {
                let place = p.get("in").and_then(Value::as_str).unwrap_or("");
                let required = p.get("required").and_then(Value::as_bool) == Some(true);
                out.push(format!(
                    "parameter {name} in {place}{}",
                    if required { " required" } else { "" }
                ));
            }
        }
    }
    if op.get("requestBody").is_some() {
        out.push("takes a request body".into());
    }
    if let Some(responses) = op.get("responses").and_then(Value::as_object) {
        for (code, r) in responses {
            let what = r.get("description").and_then(Value::as_str).unwrap_or("");
            out.push(format!("response {code} {what}"));
        }
    }
    if op.get("deprecated").and_then(Value::as_bool) == Some(true) {
        out.push("deprecated".into());
    }
    Some(out.join("\n"))
}

/// One chunk for each operation. `openapi` adds what a description file says.
// spec: REQ-APIINDEX-0001
pub fn chunks(graph: &Graph, ops: &[Operation], openapi: Option<&Value>) -> Vec<Chunk> {
    ops.iter()
        .map(|op| {
            let card = api::card(graph, op);
            let mut t = format!("{} [{}]\n", op.call(), op.tag);
            if !op.summary.is_empty() {
                t.push_str(&format!("{}\n", op.summary));
            }
            t.push_str(&format!(
                "requirement {} ({})\n",
                title_of(graph, &op.node),
                card.status.as_deref().unwrap_or("no status")
            ));
            for (name, steps) in &op.scenarios {
                t.push_str(&format!("scenario {name}: {}\n", steps.join("; ")));
            }
            for (label, ids) in [
                ("product requirements", &card.prds),
                ("decision", &card.decisions),
                ("tech debt", &card.debt),
                ("test plan", &card.plans),
            ] {
                for id in ids {
                    t.push_str(&format!("{label} {}\n", title_of(graph, id)));
                }
            }
            for (id, level) in &card.risks {
                t.push_str(&format!(
                    "risk {} ({})\n",
                    title_of(graph, id),
                    level.unwrap_or("no level")
                ));
            }
            if let Some(extra) = openapi.and_then(|d| openapi_text(d, &op.method, &op.path)) {
                t.push_str(&format!("openapi\n{extra}\n"));
            }
            Chunk {
                key: format!("{} {}", op.method, op.path),
                text: t,
                node: op.node.clone(),
                call: op.call(),
                tag: op.tag.clone(),
                status: card.status.clone(),
            }
        })
        .collect()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Entry {
    pub key: String,
    pub hash: String,
    pub text: String,
    pub node: String,
    pub call: String,
    pub tag: String,
    pub status: Option<String>,
    pub vector: Vec<f32>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Store {
    pub format: u32,
    /// The embedder that made the vectors.
    pub embedder: String,
    pub entries: Vec<Entry>,
}

impl Store {
    pub fn new(embedder: &str) -> Store {
        Store {
            format: FORMAT,
            embedder: embedder.to_string(),
            entries: Vec::new(),
        }
    }
}

#[derive(Debug, Default, PartialEq, Eq)]
pub struct SyncStats {
    pub added: usize,
    pub updated: usize,
    pub removed: usize,
    pub unchanged: usize,
}

fn path_of(root: &Path) -> PathBuf {
    root.join(INDEX_PATH)
}

/// Reads the index. A missing, damaged or older file is no index.
pub fn load(root: &Path) -> Option<Store> {
    let text = fs::read_to_string(path_of(root)).ok()?;
    let store: Store = serde_json::from_str(&text).ok()?;
    (store.format == FORMAT).then_some(store)
}

pub fn save(root: &Path, store: &Store) -> std::io::Result<()> {
    let path = path_of(root);
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }
    fs::write(path, serde_json::to_string(store)?)
}

fn hash_of(chunk: &Chunk, embedder: &str) -> String {
    let mut h = blake3::Hasher::new();
    h.update(chunk.text.as_bytes());
    h.update(b"\0");
    h.update(embedder.as_bytes());
    h.finalize().to_hex().to_string()
}

/// Brings the index in step with the chunks: embeds the new and changed
/// ones, keeps the rest, and drops the operations that are gone. A different
/// embedder starts the index again.
// spec: REQ-APIINDEX-0002
pub fn sync(
    store: &mut Store,
    chunks: Vec<Chunk>,
    embedder: &dyn Embedder,
) -> Result<SyncStats, String> {
    let id = embedder.id();
    let mut stats = SyncStats::default();
    if store.embedder != id {
        store.entries.clear();
        store.embedder = id.clone();
    }
    let old: std::collections::BTreeMap<String, Entry> = std::mem::take(&mut store.entries)
        .into_iter()
        .map(|e| (e.key.clone(), e))
        .collect();
    let mut kept: Vec<Option<Entry>> = Vec::new();
    let mut to_embed: Vec<(usize, String)> = Vec::new();
    let mut made: Vec<Chunk> = Vec::new();
    for chunk in chunks {
        let hash = hash_of(&chunk, &id);
        match old.get(&chunk.key) {
            Some(e) if e.hash == hash => {
                stats.unchanged += 1;
                kept.push(Some(Entry {
                    node: chunk.node,
                    call: chunk.call,
                    tag: chunk.tag,
                    status: chunk.status,
                    ..e.clone()
                }));
            }
            existing => {
                if existing.is_some() {
                    stats.updated += 1;
                } else {
                    stats.added += 1;
                }
                to_embed.push((kept.len(), chunk.text.clone()));
                kept.push(None);
                made.push(chunk);
            }
        }
    }
    stats.removed = old
        .keys()
        .filter(|k| {
            !made.iter().any(|c| &c.key == *k) && !kept.iter().flatten().any(|e| &e.key == *k)
        })
        .count();
    if !to_embed.is_empty() {
        let texts: Vec<String> = to_embed.iter().map(|(_, t)| t.clone()).collect();
        let vectors = embedder.embed(&texts)?;
        if vectors.len() != texts.len() {
            return Err(format!(
                "the embedder gave {} vectors for {} texts",
                vectors.len(),
                texts.len()
            ));
        }
        for (((slot, _), vector), chunk) in to_embed.iter().zip(vectors).zip(made) {
            let hash = hash_of(&chunk, &id);
            kept[*slot] = Some(Entry {
                key: chunk.key,
                hash,
                text: chunk.text,
                node: chunk.node,
                call: chunk.call,
                tag: chunk.tag,
                status: chunk.status,
                vector,
            });
        }
    }
    store.entries = kept.into_iter().flatten().collect();
    Ok(stats)
}

/// An operation that matches a query.
#[derive(Debug, Clone)]
pub struct Hit<'a> {
    pub entry: &'a Entry,
    pub score: f32,
}

/// The `top` entries nearest to a query vector, nearest first.
// spec: REQ-APIINDEX-0003
pub fn search<'a>(store: &'a Store, query: &[f32], top: usize) -> Vec<Hit<'a>> {
    let mut hits: Vec<Hit> = store
        .entries
        .iter()
        .filter(|e| e.vector.len() == query.len())
        .map(|entry| Hit {
            entry,
            score: cosine(&entry.vector, query),
        })
        .collect();
    hits.sort_by(|a, b| {
        b.score
            .total_cmp(&a.score)
            .then(a.entry.key.cmp(&b.entry.key))
    });
    hits.truncate(top);
    hits
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;

    fn chunk(key: &str, text: &str) -> Chunk {
        Chunk {
            key: key.into(),
            text: text.into(),
            node: "REQ-A-0001".into(),
            call: key.into(),
            tag: "a".into(),
            status: None,
        }
    }

    struct Counting(Cell<usize>, &'static str);
    impl Embedder for Counting {
        fn id(&self) -> String {
            self.1.into()
        }
        fn embed(&self, texts: &[String]) -> Result<Vec<Vec<f32>>, String> {
            self.0.set(self.0.get() + texts.len());
            HashEmbedder.embed(texts)
        }
    }

    // verifies: REQ-APIINDEX-0002.S1
    #[test]
    fn only_new_and_changed_chunks_are_embedded_again() {
        let embedder = Counting(Cell::new(0), "test");
        let mut store = Store::new("test");
        let first = vec![
            chunk("get /orders", "list orders"),
            chunk("post /orders", "place an order"),
            chunk("delete /orders", "clear all orders"),
        ];
        let s = sync(&mut store, first, &embedder).unwrap();
        assert_eq!((s.added, s.updated, s.removed, s.unchanged), (3, 0, 0, 0));
        assert_eq!(embedder.0.get(), 3);
        let second = vec![
            chunk("get /orders", "list orders"),
            chunk("post /orders", "place an order and send a receipt"),
            chunk("get /health", "health check"),
        ];
        let s = sync(&mut store, second, &embedder).unwrap();
        assert_eq!((s.added, s.updated, s.removed, s.unchanged), (1, 1, 1, 1));
        assert_eq!(embedder.0.get(), 5, "two more texts, not three");
        assert_eq!(store.entries.len(), 3);
    }

    // verifies: REQ-APIINDEX-0002.S2
    #[test]
    fn a_different_embedder_starts_the_index_again() {
        let mut store = Store::new("one");
        let chunks = || vec![chunk("get /a", "alpha"), chunk("get /b", "beta")];
        sync(&mut store, chunks(), &Counting(Cell::new(0), "one")).unwrap();
        let other = Counting(Cell::new(0), "two");
        let s = sync(&mut store, chunks(), &other).unwrap();
        assert_eq!((s.added, s.unchanged), (2, 0));
        assert_eq!(other.0.get(), 2);
        assert_eq!(store.embedder, "two");
    }

    // verifies: REQ-APIINDEX-0003.S1
    #[test]
    fn the_nearest_operation_comes_first() {
        let mut store = Store::new("hash");
        sync(
            &mut store,
            vec![
                chunk("get /orders/{id}", "GET /orders/{id} fetch an order by its id, responds 404 when no such order"),
                chunk("post /payments", "POST /payments charge a card for an order, responds 402 when the card is declined"),
                chunk("get /health", "GET /health report that the service is up"),
            ],
            &HashEmbedder,
        )
        .unwrap();
        let ask = |q: &str| -> Vec<String> {
            search(&store, &hash_embed(q), 3)
                .iter()
                .map(|h| h.entry.key.clone())
                .collect()
        };
        assert_eq!(ask("fetch an order")[0], "get /orders/{id}");
        assert_eq!(ask("card declined")[0], "post /payments");
        assert_eq!(ask("is the service up")[0], "get /health");
        assert_eq!(ask("orders").len(), 3);
        assert!(search(&store, &hash_embed("card"), 1).len() == 1);
        // Words and their plurals meet.
        assert_eq!(tokens("Orders were Placed"), ["order", "were", "plac"]);
    }

    // verifies: REQ-APIINDEX-0001.S1
    #[test]
    fn a_chunk_holds_the_operation_and_what_surrounds_it() {
        use crate::api::operations;
        use std::path::PathBuf;
        let dir = tempfile::tempdir().unwrap();
        let files = [
            (
                "specs/prd/1.md",
                "---\nid: PRD-0001\ntype: prd\nstatus: draft\n---\n# PRD-0001: Checkout\n",
            ),
            (
                "specs/risks/1.md",
                "---\nid: RISK-0001\ntype: risk\nstatus: open\nlikelihood: high\nimpact: high\nlinks:\n  mitigated-by: [REQ-ORDERS-0001]\n---\n# RISK-0001: Guessable order ids\n",
            ),
            (
                "specs/requirements/orders/orders.md",
                "---\nid: REQ-ORDERS-0001\ntype: requirement\nstatus: draft\nlinks:\n  refines: PRD-0001\n---\n# REQ-ORDERS-0001: Orders\n\n### Requirement: GET /orders/{id}\nThe API SHALL provide `GET /orders/{id}`: fetch an order.\n\n#### Scenario: Missing {#S1}\n- **WHEN** the order does not exist\n- **THEN** the API responds with 404: No such order\n",
            ),
        ];
        let mut sources = Vec::new();
        for (rel, text) in files {
            let path = dir.path().join(rel);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(&path, text).unwrap();
            sources.push((PathBuf::from(rel), text.as_bytes().to_vec()));
        }
        let graph = Graph::from_sources(sources);
        let ops = operations(&graph, dir.path()).unwrap();
        let doc: Value = serde_json::from_str(
            "{\"openapi\":\"3.0.0\",\"paths\":{\"/orders/{id}\":{\"get\":{\"operationId\":\"getOrder\",\"parameters\":[{\"name\":\"id\",\"in\":\"path\",\"required\":true}]}}}}",
        )
        .unwrap();
        let made = chunks(&graph, &ops, Some(&doc));
        assert_eq!(made.len(), 1);
        let t = &made[0].text;
        for want in [
            "GET /orders/{id}",
            "scenario Missing: WHEN the order does not exist; THEN the API responds with 404",
            "product requirements PRD-0001: Checkout",
            "risk RISK-0001: Guessable order ids (critical)",
            "operationId: getOrder",
            "parameter id in path required",
        ] {
            assert!(t.contains(want), "{want} in {t}");
        }
        assert_eq!(made[0].key, "get /orders/{id}");
    }
}
