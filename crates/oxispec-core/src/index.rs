//! The local index: a cache of parsed nodes under `.oxispec/cache`, so a
//! second run reparses nothing that did not change.
//!
//! The cache is best effort. If it is missing, damaged, from another
//! version, or cannot be written, everything is parsed from the files and
//! the result is the same, only slower.
//!
//! A file is reused without even being read when its size and modification
//! time match what was cached. Otherwise the file is read and hashed, and the
//! cached node is reused when the hash matches. A file modified shortly
//! before the cache was written is always hashed, because it may have
//! changed again within the same timestamp tick (git has the same rule).

use std::collections::{BTreeMap, HashSet};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::finding::{Finding, Severity};
use crate::graph::Graph;
use crate::node::Node;
use crate::parse::parse_file_text;

pub const CACHE_DIR: &str = ".oxispec/cache";
const CACHE_FILE: &str = "index.json";
/// Bump when the layout of the cache file changes.
const CACHE_VERSION: u32 = 11;
/// A different engine version may parse differently, so it never reuses a cache.
const ENGINE: &str = env!("CARGO_PKG_VERSION");
/// Files modified within this window before the cache was written are not trusted by size and time.
const RACY_WINDOW_NS: u128 = 2_000_000_000;

/// What a load did. `parsed` is the number of files that were parsed, the
/// expensive step. `read` is the number of files whose content was read.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct LoadStats {
    /// Markdown files found.
    pub files: usize,
    pub read: usize,
    pub parsed: usize,
    pub reused: usize,
    /// Files in the cache that no longer exist.
    pub removed: usize,
    pub cache_written: bool,
}

#[derive(Serialize, Deserialize)]
struct CacheFile {
    cache_version: u32,
    engine: String,
    written_at_ns: u128,
    /// Keyed by the path relative to the project root, with `/` separators.
    files: BTreeMap<String, Entry>,
}

#[derive(Serialize, Deserialize, Clone)]
struct Entry {
    size: u64,
    mtime_ns: u128,
    hash: String,
    /// `None` for markdown files that are not nodes.
    node: Option<Node>,
}

fn cache_path(root: &Path) -> PathBuf {
    root.join(CACHE_DIR).join(CACHE_FILE)
}

fn read_cache(root: &Path) -> Option<CacheFile> {
    let text = fs::read_to_string(cache_path(root)).ok()?;
    let cache: CacheFile = serde_json::from_str(&text).ok()?;
    (cache.cache_version == CACHE_VERSION && cache.engine == ENGINE).then_some(cache)
}

fn now_ns() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos())
}

fn mtime_ns(meta: &fs::Metadata) -> u128 {
    meta.modified()
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map_or(0, |d| d.as_nanos())
}

fn key_of(rel: &Path) -> String {
    rel.to_string_lossy().replace('\\', "/")
}

fn hash_of(bytes: &[u8]) -> String {
    blake3::hash(bytes).to_hex().to_string()
}

fn write_cache(root: &Path, files: BTreeMap<String, Entry>) -> io::Result<()> {
    let dir = root.join(CACHE_DIR);
    fs::create_dir_all(&dir)?;
    // Projects created before the cache existed have no ignore file for it.
    let ignore = root.join(".oxispec/.gitignore");
    if !ignore.exists() {
        let _ = fs::write(ignore, "cache/\n");
    }
    let cache = CacheFile {
        cache_version: CACHE_VERSION,
        engine: ENGINE.to_string(),
        written_at_ns: now_ns(),
        files,
    };
    let json = serde_json::to_string(&cache).map_err(io::Error::other)?;
    let tmp = dir.join(format!("{CACHE_FILE}.{}.tmp", std::process::id()));
    fs::write(&tmp, json)?;
    fs::rename(&tmp, cache_path(root)).inspect_err(|_| {
        let _ = fs::remove_file(&tmp);
    })
}

/// Deletes the cache. A missing cache is not an error.
pub fn clear(root: &Path) -> io::Result<()> {
    match fs::remove_file(cache_path(root)) {
        Err(e) if e.kind() != io::ErrorKind::NotFound => Err(e),
        _ => Ok(()),
    }
}

/// What loading one file came to.
#[derive(Default)]
struct Processed {
    node: Option<Node>,
    entry: Option<Entry>,
    findings: Vec<Finding>,
    read: bool,
    parsed: bool,
    reused: bool,
    /// The cache has to be written again because of this file.
    dirty: bool,
}

/// Loads one file: from the cache when its size and time match, else by reading it.
fn process(path: &Path, key: &str, old: Option<&CacheFile>, written_at: u128) -> Processed {
    let rel_buf = PathBuf::from(key);
    let rel = rel_buf.as_path();
    let cached = old.and_then(|c| c.files.get(key));
    let meta = fs::metadata(path).ok();
    let mut out = Processed::default();

    // Same size and time as when it was cached, and not too recent to trust.
    if let (Some(entry), Some(meta)) = (cached, &meta) {
        let mtime = mtime_ns(meta);
        if mtime != 0
            && entry.size == meta.len()
            && entry.mtime_ns == mtime
            && mtime + RACY_WINDOW_NS < written_at
        {
            out.reused = true;
            out.node = entry.node.clone().map(|n| Node {
                path: rel.to_path_buf(),
                ..n
            });
            out.entry = Some(entry.clone());
            return out;
        }
    }

    let bytes = match fs::read(path) {
        Ok(bytes) => bytes,
        Err(e) => {
            out.findings.push(
                Finding::new("W003", Severity::Warning, format!("cannot read file: {e}"))
                    .at(rel, None),
            );
            out.dirty = true;
            return out;
        }
    };
    out.read = true;
    let hash = hash_of(&bytes);
    let size = bytes.len() as u64;
    let mtime = meta.as_ref().map_or(0, mtime_ns);

    if let Some(entry) = cached
        && entry.hash == hash
    {
        out.reused = true;
        out.node = entry.node.clone().map(|n| Node {
            path: rel.to_path_buf(),
            ..n
        });
        out.entry = Some(Entry {
            size,
            mtime_ns: mtime,
            hash,
            node: entry.node.clone(),
        });
        // Rewriting moves the cache's write time forward, so this file is
        // trusted by size and time the next time.
        out.dirty = true;
        return out;
    }

    let Ok(text) = String::from_utf8(bytes) else {
        out.findings.push(
            Finding::new(
                "W003",
                Severity::Warning,
                "cannot read file: not valid UTF-8",
            )
            .at(rel, None),
        );
        out.dirty = true;
        return out;
    };
    let (node, parse_findings) = parse_file_text(rel, &text);
    out.parsed = true;
    out.dirty = true;
    // Files with findings are parsed every time, so their findings never go stale.
    if parse_findings.is_empty() {
        out.entry = Some(Entry {
            size,
            mtime_ns: mtime,
            hash,
            node: node.clone(),
        });
    }
    out.findings = parse_findings;
    out.node = node;
    out
}

/// Loads every node under `root`. Hidden folders and anything ignored by
/// `.gitignore` are skipped, so `.oxispec/` and `target/` are not read.
///
/// With `use_cache`, unchanged files are not parsed again and the cache is
/// updated when something changed.
/// The last graph a long-running process loaded, and what the files looked like then.
struct Memo {
    root: PathBuf,
    fingerprint: u64,
    graph: Graph,
    /// When the files were last compared in full.
    verified: Instant,
}

/// How long a graph is trusted on the word of the file watcher alone. After
/// that the files are compared again, in case an event was missed.
const TRUST: Duration = Duration::from_secs(30);

static MEMO: Mutex<Option<Memo>> = Mutex::new(None);
static MEMO_ON: AtomicBool = AtomicBool::new(false);
/// Set when something under the project may have changed since the memo was built.
static DIRTY: AtomicBool = AtomicBool::new(true);
static WATCHER: Mutex<Option<notify::RecommendedWatcher>> = Mutex::new(None);
static WATCHING: AtomicBool = AtomicBool::new(false);

/// Whether this process keeps graphs in memory (a running server).
pub(crate) fn keeps_graph() -> bool {
    MEMO_ON.load(Ordering::Relaxed)
}

/// Lets [`load`] keep the last graph in memory and hand it out again while no
/// markdown file has changed. A long-running process (the MCP server) turns
/// this on; a command that runs once has nothing to gain.
///
/// A file watcher tells the process when something changed, so a call that
/// finds nothing changed does not walk the project. If the watcher cannot
/// start, every call compares the files instead.
pub fn keep_graph_in_memory(root: &Path) {
    use notify::{RecursiveMode, Watcher};
    MEMO_ON.store(true, Ordering::Relaxed);
    // Some systems report paths in their canonical form, so skip both spellings.
    let canonical = fs::canonicalize(root).unwrap_or_else(|_| root.to_path_buf());
    let skip: Vec<PathBuf> = [root, canonical.as_path()]
        .iter()
        .flat_map(|r| [r.join(".oxispec"), r.join(".git")])
        .collect();
    // The config holds the format profiles, so a change to it counts.
    let config: Vec<PathBuf> = [root, canonical.as_path()]
        .iter()
        .map(|r| r.join(crate::config::CONFIG_PATH))
        .collect();
    let watcher = notify::recommended_watcher(move |event: notify::Result<notify::Event>| {
        match event {
            Ok(event) => {
                if event
                    .paths
                    .iter()
                    .any(|p| config.contains(p) || !skip.iter().any(|s| p.starts_with(s)))
                {
                    DIRTY.store(true, Ordering::Release);
                }
            }
            // Something went wrong: do not trust the watcher until the next full comparison.
            Err(_) => DIRTY.store(true, Ordering::Release),
        }
    })
    .and_then(|mut w| w.watch(root, RecursiveMode::Recursive).map(|()| w));
    if let Ok(w) = watcher {
        *WATCHER.lock().unwrap_or_else(|e| e.into_inner()) = Some(w);
        WATCHING.store(true, Ordering::Relaxed);
    }
}

/// Forgets that the files were unchanged. Commands that write call this, so
/// the next load does not wait for the watcher to say so.
pub fn graph_may_have_changed() {
    DIRTY.store(true, Ordering::Release);
}

/// A number that changes when a markdown file is added, removed, resized or
/// modified. `None` when a file changed in the last two seconds, because a
/// second edit in the same clock tick could then go unseen.
fn fingerprint(root: &Path, found: &[(PathBuf, String)]) -> Option<u64> {
    let now = now_ns();
    let mut hasher = blake3::Hasher::new();
    for (path, key) in found {
        let meta = fs::metadata(path).ok()?;
        let mtime = mtime_ns(&meta);
        if now.saturating_sub(mtime) < RACY_WINDOW_NS {
            return None;
        }
        hasher.update(key.as_bytes());
        hasher.update(&meta.len().to_le_bytes());
        hasher.update(&mtime.to_le_bytes());
    }
    // The format profiles in the config change what the checks say.
    if let Ok(meta) = fs::metadata(root.join(crate::config::CONFIG_PATH)) {
        let mtime = mtime_ns(&meta);
        if now.saturating_sub(mtime) < RACY_WINDOW_NS {
            return None;
        }
        hasher.update(&meta.len().to_le_bytes());
        hasher.update(&mtime.to_le_bytes());
    }
    let digest = hasher.finalize();
    Some(u64::from_le_bytes(digest.as_bytes()[..8].try_into().ok()?))
}

fn walk(root: &Path) -> Vec<(PathBuf, String)> {
    let walker = ignore::WalkBuilder::new(root).require_git(false).build();
    let mut found: Vec<(PathBuf, String)> = Vec::new();
    for dirent in walker.flatten() {
        let path = dirent.path();
        let is_markdown = path.extension().is_some_and(|e| e == "md");
        if !is_markdown || !dirent.file_type().is_some_and(|t| t.is_file()) {
            continue;
        }
        // Paths are kept with `/` separators on every platform, so output and
        // JSON look the same everywhere and match what git reports.
        let key = key_of(path.strip_prefix(root).unwrap_or(path));
        found.push((path.to_path_buf(), key));
    }
    found
}

pub fn load(root: &Path, use_cache: bool) -> (Graph, LoadStats) {
    let memo_on = use_cache && MEMO_ON.load(Ordering::Relaxed);
    if memo_on
        && WATCHING.load(Ordering::Relaxed)
        && !DIRTY.load(Ordering::Acquire)
        && let Ok(guard) = MEMO.lock()
        && let Some(memo) = guard.as_ref()
        && memo.root == root
        && memo.verified.elapsed() < TRUST
    {
        let files = memo.graph.nodes.len();
        let stats = LoadStats {
            files,
            reused: files,
            ..LoadStats::default()
        };
        return (memo.graph.clone(), stats);
    }
    // Changes that arrive while this load runs must leave the flag set.
    DIRTY.store(false, Ordering::Release);

    // Walking is cheap. Reading, hashing and parsing wait on the disk and the CPU, so they
    // are spread over a few threads. A cold start on a big project is mostly that waiting.
    let found = walk(root);
    let print = if memo_on {
        fingerprint(root, &found)
    } else {
        None
    };
    if let Some(print) = print
        && let Ok(mut guard) = MEMO.lock()
        && let Some(memo) = guard.as_mut()
        && memo.root == root
        && memo.fingerprint == print
    {
        memo.verified = Instant::now();
        let stats = LoadStats {
            files: found.len(),
            reused: found.len(),
            ..LoadStats::default()
        };
        return (memo.graph.clone(), stats);
    }
    let (graph, stats) = load_files(root, use_cache, found);
    let graph = graph
        .with_formats(crate::formats::Formats::load(root))
        .with_debt(
            crate::config::load(root)
                .map(|c| c.debt)
                .unwrap_or_default(),
        );
    if let Some(print) = print
        && let Ok(mut guard) = MEMO.lock()
    {
        *guard = Some(Memo {
            root: root.to_path_buf(),
            fingerprint: print,
            graph: graph.clone(),
            verified: Instant::now(),
        });
    } else {
        DIRTY.store(true, Ordering::Release);
    }
    (graph, stats)
}

fn load_files(root: &Path, use_cache: bool, found: Vec<(PathBuf, String)>) -> (Graph, LoadStats) {
    let old = if use_cache { read_cache(root) } else { None };
    let written_at = old.as_ref().map_or(0, |c| c.written_at_ns);
    let mut stats = LoadStats::default();
    let mut nodes: Vec<Node> = Vec::new();
    let mut findings: Vec<Finding> = Vec::new();
    let mut entries: BTreeMap<String, Entry> = BTreeMap::new();
    let mut seen: HashSet<String> = HashSet::new();
    let mut dirty = false;

    stats.files = found.len();

    let workers = std::thread::available_parallelism()
        .map_or(1, |n| n.get())
        .clamp(1, 8);
    let chunk = found.len().div_ceil(workers).max(1);
    let processed: Vec<Processed> = std::thread::scope(|scope| {
        let handles: Vec<_> = found
            .chunks(chunk)
            .map(|part| {
                let old = old.as_ref();
                scope.spawn(move || {
                    part.iter()
                        .map(|(path, key)| process(path, key, old, written_at))
                        .collect::<Vec<_>>()
                })
            })
            .collect();
        handles
            .into_iter()
            .flat_map(|h| h.join().unwrap_or_default())
            .collect()
    });
    for (done, (_, key)) in processed.into_iter().zip(&found) {
        seen.insert(key.clone());
        stats.read += usize::from(done.read);
        stats.parsed += usize::from(done.parsed);
        stats.reused += usize::from(done.reused);
        dirty |= done.dirty;
        if let Some(entry) = done.entry {
            entries.insert(key.clone(), entry);
        }
        findings.extend(done.findings);
        nodes.extend(done.node);
    }

    if let Some(old) = &old {
        stats.removed = old.files.keys().filter(|k| !seen.contains(*k)).count();
        dirty |= stats.removed > 0;
    }
    if use_cache && dirty {
        stats.cache_written = write_cache(root, entries).is_ok();
    }
    (Graph::new(nodes, findings), stats)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::OpenOptions;
    use std::time::Duration;

    fn node_text(id: &str) -> String {
        format!(
            "---\nid: {id}\ntype: requirement\n---\n### Requirement: X\nIt SHALL work.\n\n#### Scenario: S {{#S1}}\n- **WHEN** a\n- **THEN** b\n"
        )
    }

    fn write(root: &Path, rel: &str, text: &str) -> PathBuf {
        let path = root.join(rel);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, text).unwrap();
        path
    }

    /// Makes a file look like it was last modified an hour ago.
    fn age(path: &Path) {
        let file = OpenOptions::new().write(true).open(path).unwrap();
        file.set_modified(SystemTime::now() - Duration::from_secs(3600))
            .unwrap();
    }

    fn ids(graph: &Graph) -> Vec<&str> {
        graph.nodes.iter().map(|n| n.id.as_str()).collect()
    }

    #[test]
    fn a_second_load_of_unchanged_files_reads_and_parses_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        age(&write(root, "a.md", &node_text("REQ-A-0001")));
        age(&write(root, "b.md", &node_text("REQ-A-0002")));
        age(&write(root, "README.md", "# Not a node\n"));

        let (graph, first) = load(root, true);
        assert_eq!(ids(&graph), vec!["REQ-A-0001", "REQ-A-0002"]);
        assert_eq!((first.files, first.parsed, first.read), (3, 3, 3));
        assert!(first.cache_written);

        let (graph, second) = load(root, true);
        assert_eq!(ids(&graph), vec!["REQ-A-0001", "REQ-A-0002"]);
        assert_eq!(
            (second.parsed, second.read, second.reused, second.removed),
            (0, 0, 3, 0)
        );
        assert!(!second.cache_written);
    }

    #[test]
    fn files_modified_just_before_the_cache_was_written_are_hashed_not_trusted() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        write(root, "a.md", &node_text("REQ-A-0001"));
        let (_, first) = load(root, true);
        assert_eq!(first.parsed, 1);

        // The file is recent, so it is read and hashed, but still not parsed again.
        let (graph, second) = load(root, true);
        assert_eq!(ids(&graph), vec!["REQ-A-0001"]);
        assert_eq!((second.read, second.parsed, second.reused), (1, 0, 1));
    }

    #[test]
    fn a_changed_file_is_parsed_again_and_a_deleted_file_is_dropped() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        let a = write(root, "a.md", &node_text("REQ-A-0001"));
        let b = write(root, "b.md", &node_text("REQ-A-0002"));
        age(&a);
        age(&b);
        load(root, true);

        fs::write(&a, node_text("REQ-A-0099")).unwrap();
        fs::remove_file(&b).unwrap();
        let (graph, stats) = load(root, true);
        assert_eq!(ids(&graph), vec!["REQ-A-0099"]);
        assert_eq!((stats.parsed, stats.removed), (1, 1));
        assert!(stats.cache_written);

        // The new state is what the next load sees.
        let (graph, stats) = load(root, true);
        assert_eq!(ids(&graph), vec!["REQ-A-0099"]);
        assert_eq!((stats.parsed, stats.removed), (0, 0));
    }

    #[test]
    fn a_damaged_or_foreign_cache_is_ignored() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        age(&write(root, "a.md", &node_text("REQ-A-0001")));
        load(root, true);

        fs::write(cache_path(root), "not json").unwrap();
        let (graph, stats) = load(root, true);
        assert_eq!(ids(&graph), vec!["REQ-A-0001"]);
        assert_eq!(stats.parsed, 1);

        // A cache from another engine version is not reused either.
        let text = fs::read_to_string(cache_path(root)).unwrap();
        fs::write(
            cache_path(root),
            text.replace(&format!("\"engine\":\"{ENGINE}\""), "\"engine\":\"9.9.9\""),
        )
        .unwrap();
        let (_, stats) = load(root, true);
        assert_eq!(stats.parsed, 1);
        let (_, stats) = load(root, true);
        assert_eq!(stats.parsed, 0);
    }

    #[test]
    fn files_with_findings_are_parsed_every_time() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        age(&write(root, "bad.md", "---\nid: [unclosed\n---\n"));
        for _ in 0..2 {
            let (graph, stats) = load(root, true);
            assert_eq!(graph.findings.len(), 1);
            assert_eq!(graph.findings[0].code, "W002");
            assert_eq!(stats.parsed, 1);
        }
    }

    #[test]
    fn without_the_cache_nothing_is_written() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        write(root, "a.md", &node_text("REQ-A-0001"));
        let (graph, stats) = load(root, false);
        assert_eq!(ids(&graph), vec!["REQ-A-0001"]);
        assert!(!stats.cache_written);
        assert!(!root.join(CACHE_DIR).exists());
        assert_eq!(ids(&Graph::load(root)), vec!["REQ-A-0001"]);
    }

    #[test]
    fn the_cache_folder_is_ignored_by_git_and_can_be_cleared() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        age(&write(root, "a.md", &node_text("REQ-A-0001")));
        load(root, true);
        let ignore = fs::read_to_string(root.join(".oxispec/.gitignore")).unwrap();
        assert!(ignore.contains("cache/"));

        clear(root).unwrap();
        assert!(!cache_path(root).exists());
        clear(root).unwrap();
        let (_, stats) = load(root, true);
        assert_eq!(stats.parsed, 1);
    }

    #[test]
    fn the_cache_folder_itself_is_never_read_as_a_spec() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        age(&write(root, "a.md", &node_text("REQ-A-0001")));
        write(root, ".oxispec/cache/stray.md", &node_text("REQ-A-0666"));
        let (graph, _) = load(root, true);
        assert_eq!(ids(&graph), vec!["REQ-A-0001"]);
    }

    #[test]
    fn many_files_load_the_same_cold_and_warm_whatever_the_thread_count() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        for i in 0..300 {
            write(
                root,
                &format!("specs/r{i}.md"),
                &format!(
                    "---\nid: REQ-A-{i:04}\ntype: requirement\n---\n### Requirement: R{i}\nIt SHALL.\n\n#### Scenario: S\n- a\n"
                ),
            );
        }
        write(root, "notes/plain.md", "no frontmatter\n");
        write(root, "specs/bad.md", "---\nid: [\n---\n");
        let (cold, cold_stats) = load(root, true);
        let (warm, warm_stats) = load(root, true);
        let ids = |g: &Graph| g.nodes.iter().map(|n| n.id.clone()).collect::<Vec<_>>();
        assert_eq!(ids(&cold).len(), 300);
        assert_eq!(ids(&cold), ids(&warm));
        assert_eq!(cold_stats.files, 302);
        assert_eq!(cold_stats.parsed, 302);
        assert_eq!(warm_stats.files, 302);
        // The broken file is parsed again every time, so its finding stays fresh.
        assert_eq!(warm.findings.len(), 1);
        assert_eq!(cold.findings.len(), 1);
    }

    #[test]
    fn cached_nodes_get_the_path_of_the_file_they_were_found_at() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        age(&write(root, "specs/auth/a.md", &node_text("REQ-A-0001")));
        load(root, true);
        let (graph, stats) = load(root, true);
        assert_eq!(stats.reused, 1);
        assert_eq!(
            graph.nodes[0].path,
            Path::new("specs").join("auth").join("a.md")
        );
    }

    #[test]
    fn paths_use_forward_slashes_on_every_platform() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        write(root, "specs/auth/a.md", &node_text("REQ-A-0001"));
        write(root, "specs/bad.md", "---\nid: [unclosed\n---\n");
        for cached in [false, true, true] {
            let (graph, _) = load(root, cached);
            assert_eq!(graph.nodes[0].path.to_string_lossy(), "specs/auth/a.md");
            let finding = graph.findings[0].path.as_ref().unwrap();
            assert_eq!(finding.to_string_lossy(), "specs/bad.md");
        }
    }
}
