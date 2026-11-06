//! Full-text search over the nodes, with tantivy.
//!
//! The index is derived: it lives under `.oxispec/cache/search`, is brought
//! up to date from the graph on every search (only changed nodes are read
//! again), and can be deleted at any time. Without a cache it is built in
//! memory.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};

use serde::{Deserialize, Serialize};
use tantivy::collector::TopDocs;
use tantivy::query::{BooleanQuery, Occur, Query, QueryParser, TermQuery};
use tantivy::schema::{
    Field, IndexRecordOption, STORED, STRING, Schema, TEXT, TantivyDocument, TextFieldIndexing,
    TextOptions, Value,
};
use tantivy::snippet::SnippetGenerator;
use tantivy::tokenizer::{
    Language, LowerCaser, RemoveLongFilter, SimpleTokenizer, Stemmer, TextAnalyzer,
};
use tantivy::{Index, IndexReader, IndexWriter, Term, doc};

use crate::finding::Finding;
use crate::graph::Graph;
use crate::node::Node;
use crate::parse::split_frontmatter;

/// Where the index lives, relative to the project root.
pub const SEARCH_DIR: &str = ".oxispec/cache/search";
const MANIFEST: &str = "manifest.json";
const WRITER_MEMORY: usize = 50_000_000;
/// Bump when the fields or what is stored in them change.
const SEARCH_VERSION: u32 = 1;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("search index: {0}")]
    Index(String),
    #[error("search index: {0}")]
    Io(#[from] std::io::Error),
}

impl From<tantivy::TantivyError> for Error {
    fn from(e: tantivy::TantivyError) -> Self {
        Error::Index(e.to_string())
    }
}

/// What to look for.
#[derive(Debug, Clone)]
pub struct SearchQuery {
    pub text: String,
    /// Only nodes of one of these types, for example `requirement`. Empty: any.
    /// This narrows the index query itself, so nothing else is ranked or counted.
    pub types: Vec<String>,
    pub status: Option<String>,
    pub limit: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct Hit {
    pub id: String,
    #[serde(rename = "type")]
    pub ty: String,
    pub status: Option<String>,
    pub title: Option<String>,
    pub path: PathBuf,
    pub score: f32,
    /// A piece of the text around the match, with the matched words in `**`.
    pub snippet: Option<String>,
}

/// How much of the index had to be redone for this search.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct SyncStats {
    pub added: usize,
    pub updated: usize,
    pub removed: usize,
    pub unchanged: usize,
}

impl SyncStats {
    pub fn changed(&self) -> usize {
        self.added + self.updated + self.removed
    }
}

#[derive(Clone, Copy)]
struct Fields {
    path: Field,
    id: Field,
    ids: Field,
    ty: Field,
    status: Field,
    title: Field,
    body: Field,
}

/// English text: lowercase words reduced to their stems, so `sessions`
/// finds `session`. Other languages still match on whole words.
const STEMMED: &str = "en_stem";

fn register_tokenizers(index: &Index) {
    let analyzer = TextAnalyzer::builder(SimpleTokenizer::default())
        .filter(RemoveLongFilter::limit(40))
        .filter(LowerCaser)
        .filter(Stemmer::new(Language::English))
        .build();
    index.tokenizers().register(STEMMED, analyzer);
}

fn stemmed_text() -> TextOptions {
    TextOptions::default()
        .set_indexing_options(
            TextFieldIndexing::default()
                .set_tokenizer(STEMMED)
                .set_index_option(IndexRecordOption::WithFreqsAndPositions),
        )
        .set_stored()
}

fn schema() -> (Schema, Fields) {
    let mut builder = Schema::builder();
    let fields = Fields {
        path: builder.add_text_field("path", STRING | STORED),
        id: builder.add_text_field("id", STRING | STORED),
        ids: builder.add_text_field("ids", TEXT),
        ty: builder.add_text_field("type", STRING | STORED),
        status: builder.add_text_field("status", STRING | STORED),
        title: builder.add_text_field("title", stemmed_text()),
        body: builder.add_text_field("body", stemmed_text()),
    };
    (builder.build(), fields)
}

/// What the index knew about each file when it was last updated.
#[derive(Debug, Default, Serialize, Deserialize)]
struct Manifest {
    version: u32,
    engine: String,
    /// Path to a signature of the node that was indexed.
    files: BTreeMap<String, String>,
}

impl Manifest {
    fn current(files: BTreeMap<String, String>) -> Manifest {
        Manifest {
            version: SEARCH_VERSION,
            engine: tantivy::version_string().to_string(),
            files,
        }
    }

    fn is_current(&self) -> bool {
        self.version == SEARCH_VERSION && self.engine == tantivy::version_string()
    }
}

/// Changes when anything the index holds about the node changes.
fn signature(node: &Node) -> String {
    let text = format!(
        "{}\0{}\0{}\0{}\0{}\0{}",
        node.id,
        node.ty,
        node.status.as_deref().unwrap_or(""),
        node.title.as_deref().unwrap_or(""),
        node.owner.as_deref().unwrap_or(""),
        node.body_hash
    );
    blake3::hash(text.as_bytes()).to_hex().to_string()
}

fn key(node: &Node) -> String {
    node.path.to_string_lossy().replace('\\', "/")
}

fn read_manifest(dir: &Path) -> Option<Manifest> {
    let text = fs::read_to_string(dir.join(MANIFEST)).ok()?;
    serde_json::from_str::<Manifest>(&text)
        .ok()
        .filter(Manifest::is_current)
}

fn write_manifest(dir: &Path, manifest: &Manifest) -> Result<(), Error> {
    let temporary = dir.join(format!("{MANIFEST}.tmp"));
    let text = serde_json::to_string(manifest).map_err(|e| Error::Index(e.to_string()))?;
    fs::write(&temporary, text)?;
    fs::rename(&temporary, dir.join(MANIFEST))?;
    Ok(())
}

/// Opens the stored index, or makes an empty one. `None` for the manifest
/// means the index is new or was thrown away.
fn open(dir: &Path, schema: &Schema, rebuild: bool) -> Result<(Index, Option<Manifest>), Error> {
    if rebuild {
        let _ = fs::remove_dir_all(dir);
    }
    if let Some(manifest) = read_manifest(dir)
        && let Ok(index) = Index::open_in_dir(dir.join("index"))
    {
        register_tokenizers(&index);
        return Ok((index, Some(manifest)));
    }
    let _ = fs::remove_dir_all(dir);
    fs::create_dir_all(dir.join("index"))?;
    Ok((
        Index::create_in_dir(dir.join("index"), schema.clone())?,
        None,
    ))
}

/// The text of a node: everything after the frontmatter.
fn body_of(root: &Path, node: &Node) -> String {
    let text = fs::read_to_string(root.join(&node.path)).unwrap_or_default();
    match split_frontmatter(&text) {
        Some((_, body, _)) => body.to_string(),
        None => text,
    }
}

fn add(writer: &IndexWriter, fields: &Fields, root: &Path, node: &Node) -> Result<(), Error> {
    let path = key(node);
    writer.delete_term(Term::from_field_text(fields.path, &path));
    let mut document = doc!(
        fields.path => path,
        fields.id => node.id.clone(),
        fields.ids => format!("{} {}", node.id, node.path.display()),
        fields.ty => node.ty.clone(),
        fields.body => body_of(root, node),
    );
    if let Some(status) = &node.status {
        document.add_text(fields.status, status);
    }
    if let Some(title) = &node.title {
        document.add_text(fields.title, title);
    }
    writer.add_document(document)?;
    Ok(())
}

/// Brings the index in line with the graph, reading only the files whose
/// node changed. Returns what was done and the new manifest entries.
fn sync(
    root: &Path,
    graph: &Graph,
    index: &Index,
    fields: &Fields,
    previous: Option<Manifest>,
) -> Result<(SyncStats, BTreeMap<String, String>), Error> {
    let known = previous.map(|m| m.files).unwrap_or_default();
    let mut files = BTreeMap::new();
    let mut stats = SyncStats::default();
    let mut to_index: Vec<&Node> = Vec::new();
    for node in &graph.nodes {
        let (path, sig) = (key(node), signature(node));
        if files.contains_key(&path) {
            continue;
        }
        match known.get(&path) {
            Some(old) if *old == sig => stats.unchanged += 1,
            Some(_) => {
                stats.updated += 1;
                to_index.push(node);
            }
            None => {
                stats.added += 1;
                to_index.push(node);
            }
        }
        files.insert(path, sig);
    }
    let gone: Vec<&String> = known.keys().filter(|p| !files.contains_key(*p)).collect();
    stats.removed = gone.len();

    if !to_index.is_empty() || !gone.is_empty() {
        let mut writer: IndexWriter = index.writer_with_num_threads(1, WRITER_MEMORY)?;
        for path in gone {
            writer.delete_term(Term::from_field_text(fields.path, path));
        }
        for node in to_index {
            add(&writer, fields, root, node)?;
        }
        writer.commit()?;
        writer.wait_merging_threads()?;
    }
    Ok((stats, files))
}

/// An open index that matches one memoized graph. While the graph is the
/// same, a search needs no lock, no sync and no reopening of the files.
struct Live {
    root: PathBuf,
    graph: Arc<OnceLock<Vec<Finding>>>,
    index: Index,
    reader: IndexReader,
    fields: Fields,
}

static LIVE: Mutex<Option<Live>> = Mutex::new(None);

fn live_lock() -> std::sync::MutexGuard<'static, Option<Live>> {
    LIVE.lock().unwrap_or_else(|e| e.into_inner())
}

// spec: REQ-SEARCH-0001
// spec: REQ-SEARCH-0002
/// Searches the nodes of `graph`. With `persist` the index is kept under
/// `.oxispec/cache/search` and updated in place, otherwise it is built in
/// memory for this one search. `rebuild` throws a stored index away first.
pub fn search(
    root: &Path,
    graph: &Graph,
    persist: bool,
    rebuild: bool,
    query: &SearchQuery,
) -> Result<(Vec<Hit>, SyncStats), Error> {
    if persist && !rebuild && crate::index::keeps_graph() {
        let live = live_lock();
        if let Some(l) = live.as_ref()
            && l.root == root
            && Arc::ptr_eq(&l.graph, &graph.checked)
        {
            let hits = run(&l.index, &l.reader, &l.fields, query)?;
            return Ok((hits, SyncStats::default()));
        }
    }
    let (schema, fields) = schema();
    let dir = root.join(SEARCH_DIR);
    // Two processes must not build or update the stored index at once: one would
    // delete the files the other is writing. The lock is released when `_lock` is dropped.
    let _lock = if persist {
        fs::create_dir_all(root.join(crate::index::CACHE_DIR))?;
        let file = fs::OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(false)
            .open(root.join(crate::index::CACHE_DIR).join("search.lock"))?;
        file.lock()?;
        Some(file)
    } else {
        None
    };
    let (index, previous) = if persist {
        open(&dir, &schema, rebuild)?
    } else {
        let index = Index::create_in_ram(schema);
        register_tokenizers(&index);
        (index, None)
    };
    let (stats, files) = sync(root, graph, &index, &fields, previous)?;
    if persist && (stats.changed() > 0 || !dir.join(MANIFEST).exists()) {
        write_manifest(&dir, &Manifest::current(files))?;
    }
    let reader = index.reader()?;
    let hits = run(&index, &reader, &fields, query)?;
    if persist && crate::index::keeps_graph() {
        *live_lock() = Some(Live {
            root: root.to_path_buf(),
            graph: graph.checked.clone(),
            index,
            reader,
            fields,
        });
    }
    Ok((hits, stats))
}

fn text(document: &TantivyDocument, field: Field) -> Option<String> {
    document
        .get_first(field)
        .and_then(|v| v.as_str())
        .map(str::to_string)
}

fn run(
    index: &Index,
    reader: &IndexReader,
    fields: &Fields,
    query: &SearchQuery,
) -> Result<Vec<Hit>, Error> {
    let searcher = reader.searcher();
    let mut parser = QueryParser::for_index(index, vec![fields.ids, fields.title, fields.body]);
    parser.set_conjunction_by_default();
    parser.set_field_boost(fields.ids, 4.0);
    parser.set_field_boost(fields.title, 3.0);
    // A query with a stray quote or colon is still a query: search for what parses.
    let (parsed, _problems) = parser.parse_query_lenient(&query.text);

    let mut clauses: Vec<(Occur, Box<dyn Query>)> = vec![(Occur::Must, parsed)];
    if !query.types.is_empty() {
        let any_type: Vec<(Occur, Box<dyn Query>)> = query
            .types
            .iter()
            .map(|ty| {
                let term = Term::from_field_text(fields.ty, ty);
                let query: Box<dyn Query> =
                    Box::new(TermQuery::new(term, IndexRecordOption::Basic));
                (Occur::Should, query)
            })
            .collect();
        clauses.push((Occur::Must, Box::new(BooleanQuery::new(any_type))));
    }
    if let Some(value) = &query.status {
        let term = Term::from_field_text(fields.status, value);
        clauses.push((
            Occur::Must,
            Box::new(TermQuery::new(term, IndexRecordOption::Basic)),
        ));
    }
    let combined = BooleanQuery::new(clauses);
    let found = searcher.search(
        &combined,
        &TopDocs::with_limit(query.limit.max(1)).order_by_score(),
    )?;
    let snippets = SnippetGenerator::create(&searcher, &combined, fields.body)?;

    let mut hits = Vec::with_capacity(found.len());
    for (score, address) in found {
        let document: TantivyDocument = searcher.doc(address)?;
        let snippet = snippets.snippet_from_doc(&document);
        let snippet = (!snippet.fragment().is_empty()).then(|| highlight(&snippet));
        hits.push(Hit {
            id: text(&document, fields.id).unwrap_or_default(),
            ty: text(&document, fields.ty).unwrap_or_default(),
            status: text(&document, fields.status),
            title: text(&document, fields.title),
            path: PathBuf::from(text(&document, fields.path).unwrap_or_default()),
            score,
            snippet,
        });
    }
    Ok(hits)
}

/// The fragment on one line, with the matched words in `**`.
fn highlight(snippet: &tantivy::snippet::Snippet) -> String {
    let fragment = snippet.fragment();
    let mut out = String::new();
    let mut from = 0;
    for range in snippet.highlighted() {
        out.push_str(&fragment[from..range.start]);
        out.push_str("**");
        out.push_str(&fragment[range.clone()]);
        out.push_str("**");
        from = range.end;
    }
    out.push_str(&fragment[from..]);
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write(root: &Path, name: &str, text: &str) {
        let path = root.join(name);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, text).unwrap();
    }

    fn requirement(id: &str, status: &str, name: &str, text: &str) -> String {
        format!(
            "---\nid: {id}\ntype: requirement\nstatus: {status}\n---\n### Requirement: {name}\n{text}\n\n#### Scenario: S {{#S1}}\n- **WHEN** a\n- **THEN** b\n"
        )
    }

    fn query(text: &str) -> SearchQuery {
        SearchQuery {
            text: text.to_string(),
            types: Vec::new(),
            status: None,
            limit: 10,
        }
    }

    fn ids(hits: &[Hit]) -> Vec<&str> {
        hits.iter().map(|h| h.id.as_str()).collect()
    }

    fn project() -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        write(
            root,
            "specs/a.md",
            &requirement(
                "REQ-A-0001",
                "draft",
                "Session timeout",
                "The system SHALL end an idle administrator session.",
            ),
        );
        write(
            root,
            "specs/b.md",
            &requirement(
                "REQ-A-0002",
                "accepted",
                "Password length",
                "The system SHALL reject short passwords.",
            ),
        );
        write(
            root,
            "specs/adr/0001.md",
            "---\nid: ADR-0001\ntype: adr\n---\n# ADR-0001: Use Postgres\n\nThe system stores sessions in Postgres.\n",
        );
        dir
    }

    // verifies: REQ-SEARCH-0002.S3
    #[test]
    fn searches_at_the_same_time_share_one_index_without_errors() {
        let dir = project();
        let root = dir.path().to_path_buf();
        let graph = std::sync::Arc::new(crate::graph::Graph::load(&root));
        let handles: Vec<_> = (0..8)
            .map(|_| {
                let (root, graph) = (root.clone(), graph.clone());
                std::thread::spawn(move || search(&root, &graph, true, false, &query("session")))
            })
            .collect();
        for h in handles {
            let (hits, _) = h.join().unwrap().expect("a search that does not fail");
            assert!(!hits.is_empty());
        }
    }

    // verifies: REQ-SEARCH-0001.S1
    #[test]
    fn finds_words_in_the_text_the_title_and_the_id() {
        let dir = project();
        let graph = Graph::load(dir.path());
        let ask = |text: &str| {
            search(dir.path(), &graph, false, false, &query(text))
                .unwrap()
                .0
        };

        assert_eq!(ids(&ask("idle administrator")), ["REQ-A-0001"]);
        assert_eq!(ids(&ask("postgres")), ["ADR-0001"]);
        assert_eq!(ids(&ask("REQ-A-0002")), ["REQ-A-0002"]);
        assert!(ask("nonexistentword").is_empty());
        // Every word has to match, and a stray quote does not break the query.
        assert!(ask("idle passwords").is_empty());
        assert_eq!(ids(&ask("\"session")), ["REQ-A-0001", "ADR-0001"]);
    }

    // verifies: REQ-SEARCH-0001.S2
    #[test]
    fn a_title_match_ranks_above_a_text_match() {
        let dir = project();
        let graph = Graph::load(dir.path());
        let (hits, _) = search(dir.path(), &graph, false, false, &query("session")).unwrap();
        assert_eq!(ids(&hits), ["REQ-A-0001", "ADR-0001"]);
        assert!(hits[0].score > hits[1].score);
        assert_eq!(hits[0].title, None, "requirements have no # heading");
        assert_eq!(hits[1].title.as_deref(), Some("Use Postgres"));
    }

    // verifies: REQ-SEARCH-0001.S3
    #[test]
    fn snippets_mark_the_matched_words() {
        let dir = project();
        let graph = Graph::load(dir.path());
        let (hits, _) = search(dir.path(), &graph, false, false, &query("passwords")).unwrap();
        let snippet = hits[0].snippet.as_deref().unwrap();
        assert!(snippet.contains("reject short **passwords**"), "{snippet}");
        assert!(!snippet.contains('\n'));
    }

    // verifies: REQ-SEARCH-0001.S4
    #[test]
    fn type_and_status_narrow_the_results() {
        let dir = project();
        let graph = Graph::load(dir.path());
        let ask = |ty: Option<&str>, status: Option<&str>| {
            let q = SearchQuery {
                types: ty.map(String::from).into_iter().collect(),
                status: status.map(String::from),
                ..query("system")
            };
            search(dir.path(), &graph, false, false, &q).unwrap().0
        };
        assert_eq!(ids(&ask(None, None)).len(), 3);
        assert_eq!(ids(&ask(Some("adr"), None)), ["ADR-0001"]);
        assert_eq!(ids(&ask(None, Some("accepted"))), ["REQ-A-0002"]);
        assert!(ask(Some("adr"), Some("accepted")).is_empty());
    }

    // verifies: REQ-SEARCH-0002.S1
    #[test]
    fn a_stored_index_only_redoes_what_changed() {
        let dir = project();
        let root = dir.path();
        let ask = |text: &str| {
            let graph = Graph::load(root);
            search(root, &graph, true, false, &query(text)).unwrap()
        };

        let (hits, stats) = ask("session");
        assert_eq!(hits.len(), 2);
        assert_eq!(
            stats,
            SyncStats {
                added: 3,
                updated: 0,
                removed: 0,
                unchanged: 0
            }
        );
        assert!(root.join(SEARCH_DIR).join(MANIFEST).is_file());

        let (_, stats) = ask("session");
        assert_eq!(
            stats,
            SyncStats {
                added: 0,
                updated: 0,
                removed: 0,
                unchanged: 3
            }
        );

        // Edit one file, delete one, add one.
        write(
            root,
            "specs/a.md",
            &requirement(
                "REQ-A-0001",
                "draft",
                "Session timeout",
                "The system SHALL lock the screen of an idle operator.",
            ),
        );
        fs::remove_file(root.join("specs/b.md")).unwrap();
        write(
            root,
            "specs/c.md",
            &requirement(
                "REQ-A-0003",
                "draft",
                "Audit log",
                "The system SHALL record every login.",
            ),
        );
        let (hits, stats) = ask("idle");
        assert_eq!(
            stats,
            SyncStats {
                added: 1,
                updated: 1,
                removed: 1,
                unchanged: 1
            }
        );
        assert_eq!(ids(&hits), ["REQ-A-0001"]);
        assert!(hits[0].snippet.as_deref().unwrap().contains("**idle**"));
        assert!(ask("administrator").0.is_empty(), "the old text is gone");
        assert!(ask("passwords").0.is_empty(), "the deleted file is gone");
        assert_eq!(ids(&ask("login").0), ["REQ-A-0003"]);

        // A changed status alone is picked up too.
        write(
            root,
            "specs/c.md",
            &requirement(
                "REQ-A-0003",
                "accepted",
                "Audit log",
                "The system SHALL record every login.",
            ),
        );
        let graph = Graph::load(root);
        let q = SearchQuery {
            status: Some("accepted".into()),
            ..query("login")
        };
        let (hits, stats) = search(root, &graph, true, false, &q).unwrap();
        assert_eq!(stats.updated, 1);
        assert_eq!(ids(&hits), ["REQ-A-0003"]);
    }

    // verifies: REQ-SEARCH-0002.S2
    #[test]
    fn a_damaged_or_old_index_is_rebuilt() {
        let dir = project();
        let root = dir.path();
        let graph = Graph::load(root);
        search(root, &graph, true, false, &query("session")).unwrap();

        fs::write(root.join(SEARCH_DIR).join(MANIFEST), "not json").unwrap();
        let (hits, stats) = search(root, &graph, true, false, &query("session")).unwrap();
        assert_eq!(hits.len(), 2);
        assert_eq!(stats.added, 3);

        let (_, stats) = search(root, &graph, true, true, &query("session")).unwrap();
        assert_eq!(stats.added, 3, "--rebuild starts again");

        // Without a stored index, nothing is left behind.
        let other = project();
        let graph = Graph::load(other.path());
        search(other.path(), &graph, false, false, &query("session")).unwrap();
        assert!(!other.path().join(SEARCH_DIR).exists());
    }
}
