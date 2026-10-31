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
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::finding::{Finding, Severity};
use crate::graph::Graph;
use crate::node::Node;
use crate::parse::parse_file_text;

pub const CACHE_DIR: &str = ".oxispec/cache";
const CACHE_FILE: &str = "index.json";
/// Bump when the layout of the cache file changes.
const CACHE_VERSION: u32 = 7;
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

/// Loads every node under `root`. Hidden folders and anything ignored by
/// `.gitignore` are skipped, so `.oxispec/` and `target/` are not read.
///
/// With `use_cache`, unchanged files are not parsed again and the cache is
/// updated when something changed.
pub fn load(root: &Path, use_cache: bool) -> (Graph, LoadStats) {
    let old = if use_cache { read_cache(root) } else { None };
    let written_at = old.as_ref().map_or(0, |c| c.written_at_ns);
    let mut stats = LoadStats::default();
    let mut nodes: Vec<Node> = Vec::new();
    let mut findings: Vec<Finding> = Vec::new();
    let mut entries: BTreeMap<String, Entry> = BTreeMap::new();
    let mut seen: HashSet<String> = HashSet::new();
    let mut dirty = false;

    let walker = ignore::WalkBuilder::new(root).require_git(false).build();
    for dirent in walker.flatten() {
        let path = dirent.path();
        let is_markdown = path.extension().is_some_and(|e| e == "md");
        if !is_markdown || !dirent.file_type().is_some_and(|t| t.is_file()) {
            continue;
        }
        stats.files += 1;
        // Paths are kept with `/` separators on every platform, so output and
        // JSON look the same everywhere and match what git reports.
        let key = key_of(path.strip_prefix(root).unwrap_or(path));
        let rel_buf = PathBuf::from(&key);
        let rel = rel_buf.as_path();
        seen.insert(key.clone());
        let cached = old.as_ref().and_then(|c| c.files.get(&key));
        let meta = fs::metadata(path).ok();

        // Same size and time as when it was cached, and not too recent to trust.
        if let (Some(entry), Some(meta)) = (cached, &meta) {
            let mtime = mtime_ns(meta);
            if mtime != 0
                && entry.size == meta.len()
                && entry.mtime_ns == mtime
                && mtime + RACY_WINDOW_NS < written_at
            {
                stats.reused += 1;
                nodes.extend(entry.node.clone().map(|n| Node {
                    path: rel.to_path_buf(),
                    ..n
                }));
                entries.insert(key, entry.clone());
                continue;
            }
        }

        let bytes = match fs::read(path) {
            Ok(bytes) => bytes,
            Err(e) => {
                findings.push(
                    Finding::new("W003", Severity::Warning, format!("cannot read file: {e}"))
                        .at(rel, None),
                );
                dirty = true;
                continue;
            }
        };
        stats.read += 1;
        let hash = hash_of(&bytes);
        let size = bytes.len() as u64;
        let mtime = meta.as_ref().map_or(0, mtime_ns);

        if let Some(entry) = cached
            && entry.hash == hash
        {
            stats.reused += 1;
            nodes.extend(entry.node.clone().map(|n| Node {
                path: rel.to_path_buf(),
                ..n
            }));
            entries.insert(
                key,
                Entry {
                    size,
                    mtime_ns: mtime,
                    hash,
                    node: entry.node.clone(),
                },
            );
            // Rewriting moves the cache's write time forward, so this file is
            // trusted by size and time the next time.
            dirty = true;
            continue;
        }

        let Ok(text) = String::from_utf8(bytes) else {
            findings.push(
                Finding::new(
                    "W003",
                    Severity::Warning,
                    "cannot read file: not valid UTF-8",
                )
                .at(rel, None),
            );
            dirty = true;
            continue;
        };
        let (node, parse_findings) = parse_file_text(rel, &text);
        stats.parsed += 1;
        dirty = true;
        // Files with findings are parsed every time, so their findings never go stale.
        if parse_findings.is_empty() {
            entries.insert(
                key,
                Entry {
                    size,
                    mtime_ns: mtime,
                    hash,
                    node: node.clone(),
                },
            );
        }
        findings.extend(parse_findings);
        nodes.extend(node);
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
