//! Suspect links: the code and the spec drifting apart.
//!
//! Every `spec:` or `verifies:` annotation is a link between a spec node and
//! a piece of code. When a person confirms a link, the lock file records a
//! hash of both ends. From then on the link is *suspect* if either end has
//! changed, until someone looks and confirms it again.
//!
//! The lock file is committed, so a confirmation is in git history and a
//! pull request that changes code or a spec without confirming shows up.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::check::{LinkResolution, resolve_link};
use crate::finding::Finding;
use crate::graph::Graph;
use crate::id;
use crate::trace::{Annotation, Kind};

/// Where confirmations are kept, relative to the project root.
pub const LOCK_PATH: &str = ".oxispec/trace.lock.json";
const LOCK_VERSION: u32 = 1;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("{0}: {1}")]
    Io(PathBuf, std::io::Error),
    #[error("{0} is not a valid trace lock file: {1}")]
    Parse(PathBuf, String),
    #[error("{0} was written by a newer oxispec (lock version {1}); upgrade oxispec")]
    TooNew(PathBuf, u32),
}

/// What was confirmed for one link.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Entry {
    /// The node the link points at, as written, for example `REQ-AUTH-0012.S1`.
    pub target: String,
    /// Hash of the text of that node when the link was confirmed.
    pub spec: String,
    /// Hash of the code. Absent for a language without a parser.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub code: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Lock {
    pub version: u32,
    pub links: BTreeMap<String, Entry>,
    /// What a person last said is still right about an entity, for each node that is
    /// `about` one, keyed `about NODE ENTITY`.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub reviews: BTreeMap<String, Review>,
}

/// A node that is `about` an entity was looked at when the entity was like this.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Review {
    pub entity: String,
    /// [`crate::entities::fingerprint`] of the entity at the time.
    pub fingerprint: String,
}

impl Default for Lock {
    fn default() -> Self {
        Lock {
            version: LOCK_VERSION,
            links: BTreeMap::new(),
            reviews: BTreeMap::new(),
        }
    }
}

/// Reads a lock file from its text, for example as it is in a git revision.
pub fn parse(origin: &Path, text: &str) -> Result<Lock, Error> {
    let lock: Lock = serde_json::from_str(text)
        .map_err(|e| Error::Parse(origin.to_path_buf(), e.to_string()))?;
    if lock.version > LOCK_VERSION {
        return Err(Error::TooNew(origin.to_path_buf(), lock.version));
    }
    Ok(lock)
}

/// Reads the lock file. A missing file is an empty lock.
pub fn load(root: &Path) -> Result<Lock, Error> {
    let path = root.join(LOCK_PATH);
    let text = match fs::read_to_string(&path) {
        Ok(text) => text,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Lock::default()),
        Err(e) => return Err(Error::Io(path, e)),
    };
    parse(&path, &text)
}

/// Writes the lock file, sorted, one link per entry, so diffs are small.
pub fn save(root: &Path, lock: &Lock) -> Result<(), Error> {
    let path = root.join(LOCK_PATH);
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir).map_err(|e| Error::Io(dir.to_path_buf(), e))?;
    }
    let mut text = serde_json::to_string_pretty(lock)
        .map_err(|e| Error::Parse(path.clone(), e.to_string()))?;
    text.push('\n');
    fs::write(&path, text).map_err(|e| Error::Io(path, e))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum State {
    /// Confirmed, and neither end changed.
    Ok,
    /// The spec's text changed since the confirmation.
    SuspectSpec,
    /// The code changed since the confirmation.
    SuspectCode,
    SuspectBoth,
    /// Not confirmed yet.
    Unconfirmed,
    /// Confirmed once, but the annotation is gone (removed, or its symbol was renamed).
    Orphaned,
}

impl State {
    pub fn is_suspect(self) -> bool {
        matches!(
            self,
            State::SuspectSpec | State::SuspectCode | State::SuspectBoth
        )
    }
}

/// One link between a spec node and a piece of code.
#[derive(Debug, Clone, Serialize)]
pub struct Link {
    /// What the lock file calls it.
    pub key: String,
    pub target: String,
    pub kind: Kind,
    /// Where the annotation is. Absent for an orphan.
    pub path: Option<PathBuf>,
    pub line: Option<usize>,
    /// The symbol's name, when there is one.
    pub symbol: Option<String>,
    pub state: State,
}

fn key_of(a: &Annotation, symbol: &str) -> String {
    let kind = match a.kind {
        Kind::Implements => "spec",
        Kind::Verifies => "verifies",
    };
    format!(
        "{kind} {} {} {symbol}",
        a.target,
        a.path.to_string_lossy().replace('\\', "/")
    )
}

/// The current hashes of one annotation, and its lock key.
struct Current {
    key: String,
    spec: String,
    code: Option<String>,
}

/// Annotations that point at a node that exists, with their keys and hashes.
/// A key that repeats (two anonymous tests in a file) gets `#2`, `#3`.
fn current(graph: &Graph, annotations: &[Annotation]) -> Vec<(Annotation, Current)> {
    let mut seen: BTreeMap<String, usize> = BTreeMap::new();
    let mut out = Vec::new();
    for a in annotations {
        if resolve_link(graph, &a.target) != LinkResolution::Ok {
            continue;
        }
        let Some(node) = graph.get(id::base_of(&a.target)) else {
            continue;
        };
        let symbol = a.symbol.as_ref().map_or("-", |s| s.name.as_str());
        let mut key = key_of(a, symbol);
        let n = seen.entry(key.clone()).or_insert(0);
        *n += 1;
        if *n > 1 {
            key = format!("{key} #{n}");
        }
        out.push((
            a.clone(),
            Current {
                key,
                spec: node.body_hash.clone(),
                code: a.symbol.as_ref().map(|s| s.hash.clone()),
            },
        ));
    }
    out
}

fn state_of(now: &Current, lock: &Lock) -> State {
    let Some(entry) = lock.links.get(&now.key) else {
        return State::Unconfirmed;
    };
    let spec_changed = entry.spec != now.spec;
    let code_changed = entry.code.is_some() && now.code.is_some() && entry.code != now.code;
    match (spec_changed, code_changed) {
        (false, false) => State::Ok,
        (true, false) => State::SuspectSpec,
        (false, true) => State::SuspectCode,
        (true, true) => State::SuspectBoth,
    }
}

/// The state of every link in the code, and every confirmation whose link is gone.
pub fn links(graph: &Graph, annotations: &[Annotation], lock: &Lock) -> Vec<Link> {
    let now = current(graph, annotations);
    let mut found: Vec<Link> = now
        .iter()
        .map(|(a, c)| Link {
            key: c.key.clone(),
            target: a.target.clone(),
            kind: a.kind,
            path: Some(a.path.clone()),
            line: Some(a.line),
            symbol: a.symbol.as_ref().map(|s| s.name.clone()),
            state: state_of(c, lock),
        })
        .collect();
    let alive: BTreeSet<&str> = now.iter().map(|(_, c)| c.key.as_str()).collect();
    for (key, entry) in &lock.links {
        if !alive.contains(key.as_str()) {
            let kind = if key.starts_with("verifies ") {
                Kind::Verifies
            } else {
                Kind::Implements
            };
            found.push(Link {
                key: key.clone(),
                target: entry.target.clone(),
                kind,
                path: None,
                line: None,
                symbol: None,
                state: State::Orphaned,
            });
        }
    }
    found
}

/// E016 for each suspect link, with the file and line of the annotation.
pub fn findings(links: &[Link]) -> Vec<Finding> {
    links
        .iter()
        .filter(|l| l.state.is_suspect())
        .map(|l| {
            let what = match l.state {
                State::SuspectSpec => "the spec changed",
                State::SuspectCode => "the code changed",
                _ => "the spec and the code changed",
            };
            let place = l
                .symbol
                .as_deref()
                .map_or(String::new(), |s| format!(" ({s})"));
            let finding = Finding::error(
                "E016",
                format!(
                    "link to {}{place} is suspect: {what} since it was confirmed",
                    l.target
                ),
            )
            .node(id::base_of(&l.target));
            match (&l.path, l.line) {
                (Some(path), line) => finding.at(path, line),
                _ => finding,
            }
        })
        .collect()
}

/// Whether a node that is about an entity has been looked at since the entity changed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ReviewState {
    Ok,
    /// The entity changed since the node was confirmed.
    Suspect,
    /// Not confirmed yet.
    Unconfirmed,
    /// Confirmed once, but the node no longer is about the entity.
    Orphaned,
}

/// One `about` link from a node to an entity, and whether the node still stands.
#[derive(Debug, Clone, Serialize)]
pub struct ReviewLink {
    pub key: String,
    pub node: String,
    pub entity: String,
    pub state: ReviewState,
    pub path: Option<PathBuf>,
}

fn review_key(node: &str, entity: &str) -> String {
    format!("about {node} {entity}")
}

/// The state of every `about` link to an entity, and every confirmation whose link is gone.
// spec: REQ-ENTITY-0009
pub fn reviews(graph: &Graph, lock: &Lock) -> Vec<ReviewLink> {
    let (entities, _) = crate::entities::read(graph);
    let mut found = Vec::new();
    let mut alive: BTreeSet<String> = BTreeSet::new();
    for node in &graph.nodes {
        for target in node.links.get("about").into_iter().flatten() {
            let Some(entity) = entities.iter().find(|e| e.id == id::base_of(target)) else {
                continue;
            };
            let key = review_key(&node.id, &entity.id);
            if !alive.insert(key.clone()) {
                continue;
            }
            let now = crate::entities::fingerprint(entity);
            let state = match lock.reviews.get(&key) {
                None => ReviewState::Unconfirmed,
                Some(r) if r.fingerprint == now => ReviewState::Ok,
                Some(_) => ReviewState::Suspect,
            };
            found.push(ReviewLink {
                key,
                node: node.id.clone(),
                entity: entity.id.clone(),
                state,
                path: Some(node.path.clone()),
            });
        }
    }
    for (key, review) in &lock.reviews {
        if !alive.contains(key) {
            found.push(ReviewLink {
                key: key.clone(),
                node: key.split(' ').nth(1).unwrap_or("").to_string(),
                entity: review.entity.clone(),
                state: ReviewState::Orphaned,
                path: None,
            });
        }
    }
    found
}

/// W026 for each node that is about an entity that changed since the node was confirmed.
// spec: REQ-ENTITY-0009
pub fn review_findings(links: &[ReviewLink]) -> Vec<Finding> {
    links
        .iter()
        .filter(|l| l.state == ReviewState::Suspect)
        .map(|l| {
            let finding = Finding::warning(
                "W026",
                format!(
                    "{} is about {}, which changed since it was confirmed; look at it and run `oxispec confirm {}`",
                    l.node, l.entity, l.node
                ),
            )
            .node(&l.node);
            match &l.path {
                Some(path) => finding.at(path, None),
                None => finding,
            }
        })
        .collect()
}

/// Records the entities that the nodes in `ids` (every node when `None`) are about as they
/// are now, and drops confirmations whose link is gone. Returns how many were recorded and
/// dropped.
// spec: REQ-ENTITY-0009
pub fn confirm_reviews(graph: &Graph, lock: &mut Lock, ids: Option<&[String]>) -> (usize, usize) {
    let wanted = |node: &str| ids.is_none_or(|ids| ids.iter().any(|i| i == node));
    let (entities, _) = crate::entities::read(graph);
    let links = reviews(graph, lock);
    let mut recorded = 0;
    for l in links
        .iter()
        .filter(|l| l.state != ReviewState::Ok && l.state != ReviewState::Orphaned)
    {
        if !wanted(&l.node) {
            continue;
        }
        if let Some(entity) = entities.iter().find(|e| e.id == l.entity) {
            lock.reviews.insert(
                l.key.clone(),
                Review {
                    entity: entity.id.clone(),
                    fingerprint: crate::entities::fingerprint(entity),
                },
            );
            recorded += 1;
        }
    }
    let gone: Vec<String> = links
        .iter()
        .filter(|l| l.state == ReviewState::Orphaned && wanted(&l.node))
        .map(|l| l.key.clone())
        .collect();
    let dropped = gone.len();
    for key in gone {
        lock.reviews.remove(&key);
    }
    (recorded, dropped)
}

/// What `confirm` did.
#[derive(Debug, Default, PartialEq, Eq, Serialize)]
pub struct Confirmed {
    pub confirmed: usize,
    /// Confirmations removed because their link is gone.
    pub pruned: usize,
}

/// Records the current state of the links to `ids` (every link when `None`)
/// and drops confirmations whose link is gone. Links that are already ok are left as they are.
pub fn confirm(
    graph: &Graph,
    annotations: &[Annotation],
    lock: &mut Lock,
    ids: Option<&[String]>,
) -> Confirmed {
    let wanted = |target: &str| ids.is_none_or(|ids| ids.iter().any(|i| i == id::base_of(target)));
    let now = current(graph, annotations);
    let mut result = Confirmed::default();
    for (a, c) in &now {
        if wanted(&a.target) && state_of(c, lock) != State::Ok {
            lock.links.insert(
                c.key.clone(),
                Entry {
                    target: a.target.clone(),
                    spec: c.spec.clone(),
                    code: c.code.clone(),
                },
            );
            result.confirmed += 1;
        }
    }
    let alive: BTreeSet<&str> = now.iter().map(|(_, c)| c.key.as_str()).collect();
    let gone: Vec<String> = lock
        .links
        .iter()
        .filter(|(k, e)| !alive.contains(k.as_str()) && wanted(&e.target))
        .map(|(k, _)| k.clone())
        .collect();
    for key in gone {
        lock.links.remove(&key);
        result.pruned += 1;
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse::parse_file_text;
    use crate::trace::scan_text;

    fn graph(body: &str) -> Graph {
        let text = format!(
            "---\nid: REQ-A-0001\ntype: requirement\n---\n### Requirement: A\n{body}\n#### Scenario: S {{#S1}}\n- **WHEN** a\n- **THEN** b\n"
        );
        Graph::from_nodes(
            parse_file_text(Path::new("a.md"), &text)
                .0
                .into_iter()
                .collect(),
        )
    }

    const CODE: &str = "// spec: REQ-A-0001\nfn expire() {\n    work(15);\n}\n";

    fn states(g: &Graph, code: &str, lock: &Lock) -> Vec<State> {
        let found = scan_text(Path::new("src/a.rs"), code);
        links(g, &found, lock).iter().map(|l| l.state).collect()
    }

    fn confirmed(g: &Graph, code: &str) -> Lock {
        let mut lock = Lock::default();
        let found = scan_text(Path::new("src/a.rs"), code);
        confirm(g, &found, &mut lock, None);
        lock
    }

    #[test]
    fn a_link_is_unconfirmed_then_ok_then_suspect_on_each_side() {
        let g = graph("The system SHALL end idle sessions.");
        assert_eq!(states(&g, CODE, &Lock::default()), [State::Unconfirmed]);

        let lock = confirmed(&g, CODE);
        assert_eq!(states(&g, CODE, &lock), [State::Ok]);

        // Formatting and comments are not a change.
        let tidy = "// spec: REQ-A-0001 (see also the wiki)\nfn expire() {\n\n        work(15); // minutes\n}\n";
        assert_eq!(states(&g, tidy, &lock), [State::Ok]);

        let code_changed = "// spec: REQ-A-0001\nfn expire() {\n    work(30);\n}\n";
        assert_eq!(states(&g, code_changed, &lock), [State::SuspectCode]);

        let g2 = graph("The system SHALL end idle sessions after a timeout.");
        assert_eq!(states(&g2, CODE, &lock), [State::SuspectSpec]);
        assert_eq!(states(&g2, code_changed, &lock), [State::SuspectBoth]);
    }

    #[test]
    fn findings_name_the_side_that_changed_and_where() {
        let g = graph("The system SHALL end idle sessions.");
        let lock = confirmed(&g, CODE);
        let changed = "// spec: REQ-A-0001\nfn expire() {\n    work(30);\n}\n";
        let found = scan_text(Path::new("src/a.rs"), changed);
        let f = findings(&links(&g, &found, &lock));
        assert_eq!(f.len(), 1);
        assert_eq!(f[0].code, "E016");
        assert!(
            f[0].message.contains("the code changed"),
            "{}",
            f[0].message
        );
        assert!(f[0].message.contains("(expire)"), "{}", f[0].message);
        assert_eq!(f[0].path.as_deref(), Some(Path::new("src/a.rs")));
        assert_eq!(f[0].line, Some(1));
        assert_eq!(f[0].id.as_deref(), Some("REQ-A-0001"));
        // Unconfirmed and orphaned links are not findings.
        assert!(findings(&links(&g, &[], &lock)).is_empty());
    }

    #[test]
    fn a_removed_annotation_leaves_an_orphan_that_confirm_prunes() {
        let g = graph("The system SHALL end idle sessions.");
        let mut lock = confirmed(&g, CODE);
        assert_eq!(states(&g, "fn expire() {}\n", &lock), [State::Orphaned]);

        // Renaming the function is a new link and an orphan.
        let renamed = "// spec: REQ-A-0001\nfn expire_now() {\n    work(15);\n}\n";
        let mut got = states(&g, renamed, &lock);
        got.sort_by_key(|s| *s as u8);
        assert_eq!(got.len(), 2);
        assert!(got.contains(&State::Unconfirmed) && got.contains(&State::Orphaned));

        let found = scan_text(Path::new("src/a.rs"), renamed);
        let done = confirm(&g, &found, &mut lock, None);
        assert_eq!(
            done,
            Confirmed {
                confirmed: 1,
                pruned: 1
            }
        );
        assert_eq!(states(&g, renamed, &lock), [State::Ok]);
    }

    #[test]
    fn confirm_can_be_limited_to_some_nodes_and_leaves_ok_links_alone() {
        let g = graph("The system SHALL end idle sessions.");
        let mut lock = confirmed(&g, CODE);
        let before = lock.clone();
        let found = scan_text(Path::new("src/a.rs"), CODE);
        assert_eq!(confirm(&g, &found, &mut lock, None), Confirmed::default());
        assert_eq!(lock, before);

        let changed = "// spec: REQ-A-0001\nfn expire() {\n    work(30);\n}\n";
        let found = scan_text(Path::new("src/a.rs"), changed);
        let other = ["REQ-B-0001".to_string()];
        assert_eq!(
            confirm(&g, &found, &mut lock, Some(&other)),
            Confirmed::default()
        );
        assert_eq!(links(&g, &found, &lock)[0].state, State::SuspectCode);
        let mine = ["REQ-A-0001".to_string()];
        assert_eq!(confirm(&g, &found, &mut lock, Some(&mine)).confirmed, 1);
        assert_eq!(links(&g, &found, &lock)[0].state, State::Ok);
    }

    #[test]
    fn languages_without_a_parser_are_watched_on_the_spec_side_only() {
        let g = graph("The system SHALL end idle sessions.");
        let py = "# spec: REQ-A-0001\ndef expire(): pass\n";
        let found = scan_text(Path::new("a.py"), py);
        let mut lock = Lock::default();
        confirm(&g, &found, &mut lock, None);
        assert_eq!(lock.links.values().next().unwrap().code, None);
        let edited = scan_text(
            Path::new("a.py"),
            "# spec: REQ-A-0001\ndef expire(): return 1\n",
        );
        assert_eq!(links(&g, &edited, &lock)[0].state, State::Ok);
        let g2 = graph("The system SHALL end idle sessions soon.");
        assert_eq!(links(&g2, &found, &lock)[0].state, State::SuspectSpec);
    }

    #[test]
    fn repeated_anonymous_tests_get_separate_keys_and_missing_nodes_are_skipped() {
        let g = graph("The system SHALL end idle sessions.");
        let ts = "describe('a', () => {\n  it('x [REQ-A-0001.S1]', () => { a(); });\n  it('x [REQ-A-0001.S1]', () => { b(); });\n});\n// spec: REQ-NOPE-0001\nfunction f() {}\n";
        let found = scan_text(Path::new("a.test.ts"), ts);
        let all = links(&g, &found, &Lock::default());
        assert_eq!(
            all.len(),
            2,
            "the link to a missing node is E014's business"
        );
        assert_ne!(all[0].key, all[1].key);
        assert!(all[1].key.ends_with("#2"), "{}", all[1].key);
    }

    #[test]
    fn the_lock_file_is_read_and_written_sorted_and_refuses_a_newer_version() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        assert_eq!(load(root).unwrap(), Lock::default());

        let g = graph("The system SHALL end idle sessions.");
        let lock = confirmed(&g, CODE);
        save(root, &lock).unwrap();
        assert_eq!(load(root).unwrap(), lock);
        let text = fs::read_to_string(root.join(LOCK_PATH)).unwrap();
        assert!(text.ends_with("}\n"), "{text}");
        assert!(
            text.contains("\"spec REQ-A-0001 src/a.rs expire\""),
            "{text}"
        );

        fs::write(root.join(LOCK_PATH), "not json").unwrap();
        assert!(matches!(load(root), Err(Error::Parse(..))));
        fs::write(root.join(LOCK_PATH), "{\"version\": 99, \"links\": {}}").unwrap();
        assert!(matches!(load(root), Err(Error::TooNew(_, 99))));
    }

    fn about_graph(entity_block: &str, description: &str, about: bool) -> Graph {
        let links = if about {
            "links:\n  about: [ENT-0001]\n"
        } else {
            ""
        };
        Graph::from_sources([
            (
                PathBuf::from("specs/e.md"),
                format!("---\nid: ENT-0001\ntype: entity\n---\n# Order\n\n{description}\n\n```entity\n{entity_block}```\n")
                    .into_bytes(),
            ),
            (
                PathBuf::from("specs/r.md"),
                format!("---\nid: REQ-A-0001\ntype: requirement\n{links}---\n### Requirement: A\nThe system SHALL do it.\n\n#### Scenario: S {{#S1}}\n- **WHEN** a\n- **THEN** b\n").into_bytes(),
            ),
        ])
    }

    const ORDER: &str = "name: order\nattributes:\n  total: {type: int}\n";

    fn state_of_review(g: &Graph, lock: &Lock) -> Vec<ReviewState> {
        reviews(g, lock).iter().map(|r| r.state).collect()
    }

    // verifies: REQ-ENTITY-0009.S1
    #[test]
    fn a_node_about_an_entity_is_suspect_when_the_entity_changes_until_it_is_confirmed() {
        let g = about_graph(ORDER, "What an order is.", true);
        let mut lock = Lock::default();
        assert_eq!(state_of_review(&g, &lock), [ReviewState::Unconfirmed]);
        assert!(review_findings(&reviews(&g, &lock)).is_empty());

        assert_eq!(confirm_reviews(&g, &mut lock, None), (1, 0));
        assert_eq!(state_of_review(&g, &lock), [ReviewState::Ok]);

        // A reworded description is not a change to what the entity says.
        let reworded = about_graph(ORDER, "A different description.", true);
        assert_eq!(state_of_review(&reworded, &lock), [ReviewState::Ok]);

        // An attribute is.
        let changed = about_graph(
            "name: order\nattributes:\n  total: {type: int}\n  note: {type: bool}\n",
            "What an order is.",
            true,
        );
        assert_eq!(state_of_review(&changed, &lock), [ReviewState::Suspect]);
        let found = review_findings(&reviews(&changed, &lock));
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].code, "W026");
        assert_eq!(found[0].id.as_deref(), Some("REQ-A-0001"));

        // Confirming another node leaves it alone, and confirming this one settles it.
        assert_eq!(
            confirm_reviews(&changed, &mut lock, Some(&["REQ-B-0001".to_string()])),
            (0, 0)
        );
        assert_eq!(state_of_review(&changed, &lock), [ReviewState::Suspect]);
        assert_eq!(
            confirm_reviews(&changed, &mut lock, Some(&["REQ-A-0001".to_string()])),
            (1, 0)
        );
        assert_eq!(state_of_review(&changed, &lock), [ReviewState::Ok]);
    }

    // verifies: REQ-ENTITY-0009.S2
    #[test]
    fn a_confirmation_whose_link_is_gone_is_orphaned_and_dropped() {
        let g = about_graph(ORDER, "x", true);
        let mut lock = Lock::default();
        confirm_reviews(&g, &mut lock, None);
        let unlinked = about_graph(ORDER, "x", false);
        assert_eq!(state_of_review(&unlinked, &lock), [ReviewState::Orphaned]);
        assert_eq!(confirm_reviews(&unlinked, &mut lock, None), (0, 1));
        assert!(lock.reviews.is_empty());
        // A lock written before reviews existed has none, and reads.
        let old: Lock = serde_json::from_str("{\"version\":1,\"links\":{}}").unwrap();
        assert!(old.reviews.is_empty());
    }
}
