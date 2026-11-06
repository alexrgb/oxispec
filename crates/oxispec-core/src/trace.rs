//! Trace annotations in code and tests, and what they say about coverage.
//!
//! A source file ties itself to the specs with short comments:
//!
//! ```text
//! // spec: REQ-AUTH-0012                    this code implements a requirement
//! // verifies: REQ-AUTH-0012.S1             this test checks a scenario
//! it('ends idle sessions [REQ-AUTH-0012.S1]', ...)   a scenario ID in brackets
//! ```
//!
//! This module reads them and decides. Nothing here needs a parser for the
//! language, so it works the same for every language that has comments.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

use serde::Serialize;

use crate::check::{LinkResolution, resolve_link};
use crate::config::TraceConfig;
use crate::finding::Finding;
use crate::graph::Graph;
use crate::id;
use crate::languages::Languages;
use crate::symbol::{Symbol, bind};

/// Files larger than this are not scanned. They are generated or data.
const MAX_FILE_BYTES: u64 = 1_000_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    /// Code that implements a requirement (`spec:`).
    Implements,
    /// A test that checks a requirement or one of its scenarios (`verifies:`).
    Verifies,
}

/// One reference from a file to a node.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Annotation {
    /// The ID as written, for example `REQ-AUTH-0012` or `REQ-AUTH-0012.S1`.
    pub target: String,
    pub kind: Kind,
    pub path: PathBuf,
    pub line: usize,
    /// The function, type or test the comment is about, when the language has
    /// a parser. See [`crate::symbol`].
    #[serde(skip_serializing_if = "Option::is_none")]
    pub symbol: Option<Symbol>,
}

/// What came out of walking a project.
#[derive(Debug, Default)]
pub struct Scan {
    pub annotations: Vec<Annotation>,
    /// Files that were read.
    pub files: usize,
}

/// A word that is an ID in the sense of a trace anchor: it parses as an ID
/// and starts with capital letters, as `REQ`, `ADR` and `CHG` do. Prose such
/// as `spec: see-above` is not an anchor.
fn trace_id(word: &str) -> Option<&str> {
    let word = word.trim_matches(|c: char| !c.is_ascii_alphanumeric());
    let parsed = id::parse(word)?;
    let first = parsed.base.split('-').next()?;
    first
        .bytes()
        .all(|b| b.is_ascii_uppercase() || b.is_ascii_digit())
        .then_some(word)
}

/// IDs at the start of `rest`, separated by commas or spaces, until the
/// first word that is not one.
fn leading_ids(rest: &str) -> Vec<&str> {
    let mut found = Vec::new();
    for word in rest.split(|c: char| c == ',' || c.is_whitespace()) {
        if word.is_empty() {
            continue;
        }
        match trace_id(word) {
            Some(id) => found.push(id),
            None => break,
        }
    }
    found
}

/// Whether `at` in `line` falls inside a double-quoted string, judged from the
/// quotes before it on the same line. A backslash escapes the next character and
/// `'"'` is not a quote. A string that runs over several lines is not seen.
fn in_string(line: &str, at: usize) -> bool {
    let mut inside = false;
    let mut chars = line[..at].chars().peekable();
    let mut previous = ' ';
    while let Some(c) = chars.next() {
        match c {
            '\\' if inside => {
                chars.next();
            }
            '"' if previous == '\'' && chars.peek() == Some(&'\'') => {}
            '"' => inside = !inside,
            _ => {}
        }
        previous = c;
    }
    inside
}

/// Whether what comes before a marker looks like the inside of a comment.
/// A comment leader inside a string literal does not start a comment, so a
/// string such as `"// spec: REQ-A-0001"` in a test is not an annotation.
fn in_comment(before: &str, leaders: Option<&[String]>) -> bool {
    const GENERIC: &[&str] = &["//", "#", "/*", "--", "<!--", ";"];
    const DOCSTRINGS: &[&str] = &["\"\"\"", "'''"];
    let start = before.trim_start();
    let at_leader = |l: &str| before.match_indices(l).any(|(i, _)| !in_string(before, i));
    start.starts_with(['*', '%'])
        || DOCSTRINGS.iter().any(|l| before.contains(l))
        || match leaders {
            Some(own) => own.iter().any(|l| at_leader(l)),
            None => GENERIC.iter().any(|l| at_leader(l)),
        }
}

/// With a parser: the marker is inside a comment node and is the first thing
/// in it on its line, after one comment leader. So `// spec: REQ-A-0001`
/// counts, and neither a marker in prose (`` `// spec: X` `` in a doc comment)
/// nor one in a string does.
fn in_parsed_comment(ranges: &[(usize, usize)], line_offset: usize, line: &str, at: usize) -> bool {
    let absolute = line_offset + at;
    let Some(&(from, _)) = ranges
        .iter()
        .find(|(from, to)| *from <= absolute && absolute < *to)
    else {
        return false;
    };
    let column = from.saturating_sub(line_offset).min(at);
    let lead = line[column..at].trim();
    const LEADERS: &[&str] = &["", "//", "///", "//!", "/*", "/**", "/*!", "*"];
    LEADERS.contains(&lead)
}

const MARKERS: &[(&str, Kind)] = &[
    ("spec:", Kind::Implements),
    ("Spec:", Kind::Implements),
    ("verifies:", Kind::Verifies),
    ("Verifies:", Kind::Verifies),
];

/// The annotations in one file's text.
pub fn scan_text(path: &Path, text: &str) -> Vec<Annotation> {
    scan_text_with(path, text, &Languages::default())
}

/// The annotations in one file's text, reading its language as the project's
/// settings say.
pub fn scan_text_with(path: &Path, text: &str, languages: &Languages) -> Vec<Annotation> {
    let leaders: Option<&[String]> = languages
        .for_path(path)
        .filter(|l| !l.parsed)
        .map(|l| l.leaders.as_slice());
    let mut found = Vec::new();
    // For a language with a parser, a marker counts only in a real comment.
    let mut comments: Option<Option<Vec<(usize, usize)>>> = None;
    let mut offset = 0;
    for (index, whole) in text.split_inclusive('\n').enumerate() {
        let line_offset = offset;
        offset += whole.len();
        let line = whole.trim_end_matches(['\n', '\r']);
        // Most lines have nothing. Skip them cheaply.
        if !(line.contains(':') || line.contains('[')) {
            continue;
        }
        let mut add = |target: &str, kind: Kind| {
            found.push(Annotation {
                target: target.to_string(),
                kind,
                path: path.to_path_buf(),
                line: index + 1,
                symbol: None,
            });
        };
        for (marker, kind) in MARKERS {
            let mut from = 0;
            while let Some(at) = line[from..].find(marker) {
                let start = from + at;
                from = start + marker.len();
                let boundary = line[..start]
                    .chars()
                    .next_back()
                    .is_none_or(|c| !c.is_ascii_alphanumeric());
                let counts = boundary && {
                    let ranges = comments
                        .get_or_insert_with(|| crate::symbol::comment_ranges(path, text))
                        .as_deref();
                    match ranges {
                        Some(ranges) => in_parsed_comment(ranges, line_offset, line, start),
                        None => in_comment(&line[..start], leaders),
                    }
                };
                if counts {
                    for target in leading_ids(&line[from..]) {
                        add(target, *kind);
                    }
                }
            }
        }
        // `[REQ-AUTH-0012.S1]` in a test name, only when it names a scenario.
        // Not in Rust, where a test is a function and has no title. Not in a
        // comment, and not an index such as `s["REQ-A-0001.S1"]`.
        let bracket_form = path.extension().and_then(|e| e.to_str()) != Some("rs");
        let mut rest = if bracket_form { line } else { "" };
        while let Some(open) = rest.find('[') {
            let position = line.len() - rest.len() + open;
            let before = line[..position].chars().next_back();
            let indexing =
                before.is_some_and(|c| c.is_alphanumeric() || matches!(c, '_' | ')' | ']'));
            let in_comment_text = comments
                .get_or_insert_with(|| crate::symbol::comment_ranges(path, text))
                .as_deref()
                .is_some_and(|ranges| {
                    let at = line_offset + position;
                    ranges.iter().any(|(from, to)| *from <= at && at < *to)
                });
            let skip = indexing || in_comment_text;
            rest = &rest[open + 1..];
            let Some(close) = rest.find(']') else { break };
            let inside = &rest[..close];
            let ids = leading_ids(inside);
            let whole = inside.split(',').filter(|p| !p.trim().is_empty()).count();
            if !skip
                && !ids.is_empty()
                && ids.len() == whole
                && ids
                    .iter()
                    .all(|i| id::parse(i).is_some_and(|p| p.child.is_some()))
            {
                for target in ids {
                    add(target, Kind::Verifies);
                }
            }
            rest = &rest[close + 1..];
        }
    }
    if !found.is_empty() {
        let mut lines: Vec<usize> = found.iter().map(|a| a.line).collect();
        lines.dedup();
        let symbols = bind(path, text, &lines);
        for annotation in &mut found {
            if let Ok(at) = lines.binary_search(&annotation.line) {
                annotation.symbol = symbols[at].clone();
            }
        }
    }
    found
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("invalid trace exclude pattern `{0}`: {1}")]
    Pattern(String, String),
    #[error("invalid trace settings: {0}")]
    Languages(String),
}

/// The `trace.exclude` globs, ready to test paths against.
pub fn exclusions(root: &Path, exclude: &[String]) -> Result<ignore::overrides::Override, Error> {
    let mut overrides = ignore::overrides::OverrideBuilder::new(root);
    for pattern in exclude {
        overrides
            .add(&format!("!{pattern}"))
            .map_err(|e| Error::Pattern(pattern.clone(), e.to_string()))?;
    }
    overrides
        .build()
        .map_err(|e| Error::Pattern(exclude.join(", "), e.to_string()))
}

/// Whether a file, given relative to the project root, would be scanned:
/// it is not markdown, not hidden, and not excluded. Used to scan a git
/// revision the way [`scan_tree`] scans the working tree.
pub fn is_scanned(exclusions: &ignore::overrides::Override, relative: &Path) -> bool {
    let hidden = relative
        .components()
        .any(|c| c.as_os_str().to_string_lossy().starts_with('.'));
    !hidden
        && relative.extension().is_none_or(|e| e != "md")
        && !exclusions.matched(relative, false).is_ignore()
}

/// The largest file that is read.
pub const MAX_SCANNED_BYTES: usize = MAX_FILE_BYTES as usize;

/// Walks the project and reads annotations from every text file that is not
/// ignored by git, not markdown (those are the specs), and not excluded.
pub fn scan_tree(root: &Path, exclude: &[String]) -> Result<Scan, Error> {
    let config = TraceConfig {
        exclude: exclude.to_vec(),
        ..TraceConfig::default()
    };
    scan_tree_with(root, &config)
}

/// Like [`scan_tree`], with the project's whole `trace` settings: the
/// languages it scans and how it reads their comments.
pub fn scan_tree_with(root: &Path, config: &TraceConfig) -> Result<Scan, Error> {
    let languages = Languages::from_config(config).map_err(Error::Languages)?;
    let overrides = exclusions(root, &config.exclude)?;
    let walker = ignore::WalkBuilder::new(root)
        .require_git(false)
        .overrides(overrides)
        .build_parallel();

    let found: Mutex<Vec<Annotation>> = Mutex::new(Vec::new());
    let files = AtomicUsize::new(0);
    walker.run(|| {
        Box::new(|entry| {
            let Ok(entry) = entry else {
                return ignore::WalkState::Continue;
            };
            let path = entry.path();
            if !entry.file_type().is_some_and(|t| t.is_file())
                || path.extension().is_some_and(|e| e == "md")
                || entry.metadata().is_ok_and(|m| m.len() > MAX_FILE_BYTES)
            {
                return ignore::WalkState::Continue;
            }
            let Ok(text) = fs::read_to_string(path) else {
                return ignore::WalkState::Continue;
            };
            files.fetch_add(1, Ordering::Relaxed);
            let relative = path.strip_prefix(root).unwrap_or(path);
            let relative = PathBuf::from(relative.to_string_lossy().replace('\\', "/"));
            if !languages.wants(&relative) {
                return ignore::WalkState::Continue;
            }
            let annotations = scan_text_with(&relative, &text, &languages);
            if !annotations.is_empty()
                && let Ok(mut all) = found.lock()
            {
                all.extend(annotations);
            }
            ignore::WalkState::Continue
        })
    });
    let mut scan = Scan {
        annotations: found.into_inner().unwrap_or_default(),
        files: files.load(Ordering::Relaxed),
    };
    scan.annotations
        .sort_by(|a, b| (&a.path, a.line, &a.target).cmp(&(&b.path, b.line, &b.target)));
    Ok(scan)
}

/// The `verifies` and `implements` links written in nodes (a test-case node,
/// for example) count like annotations in code.
fn from_links(graph: &Graph) -> Vec<Annotation> {
    let mut out = Vec::new();
    for node in &graph.nodes {
        for (name, kind) in [
            ("implements", Kind::Implements),
            ("verifies", Kind::Verifies),
        ] {
            for target in node.links.get(name).into_iter().flatten() {
                out.push(Annotation {
                    target: target.clone(),
                    kind,
                    path: node.path.clone(),
                    line: 1,
                    symbol: None,
                });
            }
        }
    }
    out
}

/// How well requirements and scenarios are covered.
#[derive(Debug, Default, Serialize)]
pub struct Coverage {
    pub requirements: usize,
    pub implemented: usize,
    /// Requirements nothing implements.
    pub unimplemented: Vec<String>,
    /// Scenarios that have an anchor, so a test can name them.
    pub scenarios: usize,
    pub verified: usize,
    /// Scenario IDs no test verifies, like `REQ-AUTH-0012.S1`.
    pub unverified: Vec<String>,
    /// Scenarios without a `{#anchor}`, which no test can point at.
    pub unanchored_scenarios: usize,
    /// References to nodes or scenarios that do not exist (E014, E015).
    pub findings: Vec<Finding>,
}

fn percent(part: usize, whole: usize) -> f64 {
    if whole == 0 {
        100.0
    } else {
        part as f64 * 100.0 / whole as f64
    }
}

impl Coverage {
    pub fn implemented_percent(&self) -> f64 {
        percent(self.implemented, self.requirements)
    }

    pub fn verified_percent(&self) -> f64 {
        percent(self.verified, self.scenarios)
    }
}

/// Coverage of the requirements in `graph`, from annotations in code and the
/// links written in nodes. With `status`, only requirements in that status
/// are counted, since a draft is not expected to be built yet.
///
/// A `verifies` that names a requirement, not a scenario, covers all of its scenarios.
pub fn coverage(graph: &Graph, annotations: &[Annotation], status: Option<&str>) -> Coverage {
    let mut all = annotations.to_vec();
    all.extend(from_links(graph));

    let mut result = Coverage::default();
    let mut implemented = std::collections::HashSet::new();
    let mut verified = std::collections::HashSet::new();
    for a in &all {
        let (code, text) = match resolve_link(graph, &a.target) {
            LinkResolution::Ok => {
                let parsed = id::parse(&a.target);
                let base = parsed.as_ref().map_or(a.target.as_str(), |p| p.base);
                let key = parsed.and_then(|p| p.child.map(str::to_string));
                match a.kind {
                    Kind::Implements => implemented.insert(base.to_string()),
                    Kind::Verifies => verified.insert((base.to_string(), key)),
                };
                continue;
            }
            LinkResolution::MissingNode => (
                "E014",
                format!(
                    "{} refers to {}, which does not exist",
                    a.kind.verb(),
                    a.target
                ),
            ),
            LinkResolution::MissingScenario(scenario) => (
                "E015",
                format!(
                    "{} refers to scenario {scenario} of {}, which does not exist",
                    a.kind.verb(),
                    id::base_of(&a.target)
                ),
            ),
            LinkResolution::Invalid | LinkResolution::External => continue,
        };
        // Links from nodes are checked by `check`, so only code is reported here.
        if a.line > 1 || graph.nodes.iter().all(|n| n.path != a.path) {
            result
                .findings
                .push(Finding::error(code, text).at(&a.path, Some(a.line)));
        }
    }

    for node in graph.nodes.iter().filter(|n| n.ty == "requirement") {
        if status.is_some_and(|s| node.status.as_deref() != Some(s)) {
            continue;
        }
        result.requirements += 1;
        if implemented.contains(&node.id) {
            result.implemented += 1;
        } else {
            result.unimplemented.push(node.id.clone());
        }
        let whole = verified.contains(&(node.id.clone(), None));
        for scenario in node.requirements.iter().flat_map(|r| &r.scenarios) {
            let Some(anchor) = scenario
                .anchor
                .as_deref()
                .filter(|a| id::is_valid_segment(a))
            else {
                result.unanchored_scenarios += 1;
                continue;
            };
            result.scenarios += 1;
            if whole || verified.contains(&(node.id.clone(), Some(anchor.to_string()))) {
                result.verified += 1;
            } else {
                result.unverified.push(format!("{}.{anchor}", node.id));
            }
        }
    }
    result
}

impl Kind {
    fn verb(self) -> &'static str {
        match self {
            Kind::Implements => "`spec:`",
            Kind::Verifies => "`verifies:`",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse::parse_file_text;

    fn scan(text: &str) -> Vec<(String, Kind, usize)> {
        scan_text(Path::new("x"), text)
            .into_iter()
            .map(|a| (a.target, a.kind, a.line))
            .collect()
    }

    #[test]
    fn reads_annotations_in_the_comments_of_many_languages() {
        let impl_ = |t: &str, l| (t.to_string(), Kind::Implements, l);
        let verifies = |t: &str, l| (t.to_string(), Kind::Verifies, l);
        let text = "\
// spec: REQ-AUTH-0012
func ExpireAdminSession() {}
/// spec: REQ-AUTH-0013, ADR-0007
fn a() {}
# verifies: REQ-AUTH-0012.S1
def test_idle(): pass
/* spec: REQ-A-0001 */
 * Spec: REQ-A-0002
-- spec: REQ-A-0003
<!-- verifies: REQ-A-0004.S2 -->
\"\"\"spec: REQ-A-0005\"\"\"
";
        assert_eq!(
            scan(text),
            vec![
                impl_("REQ-AUTH-0012", 1),
                impl_("REQ-AUTH-0013", 3),
                impl_("ADR-0007", 3),
                verifies("REQ-AUTH-0012.S1", 5),
                impl_("REQ-A-0001", 7),
                impl_("REQ-A-0002", 8),
                impl_("REQ-A-0003", 9),
                verifies("REQ-A-0004.S2", 10),
                impl_("REQ-A-0005", 11),
            ]
        );
    }

    #[test]
    fn scenario_ids_in_brackets_are_tests() {
        assert_eq!(
            scan("it('ends idle sessions [REQ-AUTH-0012.S1]', () => {})"),
            vec![("REQ-AUTH-0012.S1".to_string(), Kind::Verifies, 1)]
        );
        assert_eq!(
            scan("t.Run(\"x [REQ-A-0001.S1, REQ-A-0001.S2]\", f)").len(),
            2
        );
        // Not scenarios, not all IDs, or not IDs at all.
        assert!(scan("let a = [REQ-AUTH-0012];").is_empty());
        assert!(scan("let a = [REQ-A-0001.S1, 5];").is_empty());
        assert!(scan("let a = [1, 2, 3];").is_empty());
        assert!(scan("x[i] = y[j]").is_empty());
    }

    #[test]
    fn a_marker_inside_a_string_is_not_an_annotation_but_one_after_code_is() {
        // The fixtures of a test are strings that look like comments.
        for text in [
            r#"let code = "// verifies: REQ-A-0001.S1\n#[test]";"#,
            r##"let t = "# spec: REQ-A-0001";"##,
            r#"run("-- spec: REQ-A-0001")"#,
        ] {
            assert!(scan(text).is_empty(), "{text}");
        }
        // A comment after code, with strings before it, still counts.
        assert_eq!(scan(r#"let u = "http://x"; // spec: REQ-A-0001"#).len(), 1);
        assert_eq!(scan(r#"let q = '"'; // spec: REQ-A-0001"#).len(), 1);
        assert_eq!(
            scan(r#"let s = "a \" b"; // verifies: REQ-A-0001.S1"#).len(),
            1
        );
        assert_eq!(scan("# spec: REQ-A-0001").len(), 1);
    }

    #[test]
    fn prose_urls_and_lookalikes_are_not_annotations() {
        for text in [
            "let spec: Spec = load();",           // not in a comment
            "// the spec: see-above for details", // not an ID
            "// spec: do-it",                     // lower case
            "// myspec: REQ-A-0001",              // not at a word boundary
            "// spec:",                           // nothing after
            "// spec: REQ",                       // no number part
            "// see https://example.com/a:REQ-A-0001",
        ] {
            assert!(scan(text).is_empty(), "{text}");
        }
        // Text after the IDs is ignored.
        assert_eq!(scan("// spec: REQ-A-0001 and more words").len(), 1);
        assert_eq!(scan("// spec: REQ-A-0001.").len(), 1);
        // A namespace is allowed.
        assert_eq!(scan("// spec: shared:REQ-A-0001")[0].0, "shared:REQ-A-0001");
    }

    fn graph() -> Graph {
        let nodes = [
            (
                "a.md",
                "---\nid: REQ-A-0001\ntype: requirement\nstatus: accepted\n---\n### Requirement: A\nIt SHALL.\n#### Scenario: One {#S1}\n#### Scenario: Two {#S2}\n#### Scenario: No anchor\n",
            ),
            (
                "b.md",
                "---\nid: REQ-A-0002\ntype: requirement\nstatus: draft\n---\n### Requirement: B\nIt SHALL.\n#### Scenario: One {#S1}\n",
            ),
            (
                "t.md",
                "---\nid: TC-0001\ntype: test-case\nlinks:\n  verifies: REQ-A-0002.S1\n---\n# Test\n",
            ),
        ];
        Graph::from_nodes(
            nodes
                .iter()
                .filter_map(|(p, t)| parse_file_text(Path::new(p), t).0)
                .collect(),
        )
    }

    fn note(target: &str, kind: Kind, path: &str) -> Annotation {
        Annotation {
            target: target.into(),
            kind,
            path: path.into(),
            line: 7,
            symbol: None,
        }
    }

    #[test]
    fn coverage_counts_requirements_and_anchored_scenarios() {
        let g = graph();
        let none = coverage(&g, &[], None);
        assert_eq!(none.requirements, 2);
        assert_eq!(none.implemented, 0);
        assert_eq!(none.scenarios, 3);
        assert_eq!(none.unanchored_scenarios, 1);
        // A test-case node's link counts.
        assert_eq!(none.verified, 1);
        assert_eq!(none.unverified, ["REQ-A-0001.S1", "REQ-A-0001.S2"]);

        let found = [
            note("REQ-A-0001", Kind::Implements, "src/a.rs"),
            note("REQ-A-0001.S1", Kind::Verifies, "tests/a.rs"),
        ];
        let some = coverage(&g, &found, None);
        assert_eq!(some.implemented, 1);
        assert_eq!(some.unimplemented, ["REQ-A-0002"]);
        assert_eq!(some.verified, 2);
        assert_eq!(some.unverified, ["REQ-A-0001.S2"]);
        assert!((some.implemented_percent() - 50.0).abs() < 1e-9);
        assert!(some.findings.is_empty());

        // Implementing a scenario implements the requirement. Verifying the
        // requirement covers every scenario of it.
        let wide = [
            note("REQ-A-0001.S2", Kind::Implements, "src/a.rs"),
            note("REQ-A-0001", Kind::Verifies, "tests/a.rs"),
        ];
        let c = coverage(&g, &wide, None);
        assert_eq!(c.implemented, 1);
        assert_eq!(c.verified, 3);
        assert!(c.unverified.is_empty());
    }

    #[test]
    fn a_status_filter_leaves_drafts_out() {
        let g = graph();
        let c = coverage(&g, &[], Some("accepted"));
        assert_eq!(c.requirements, 1);
        assert_eq!(c.scenarios, 2);
        assert_eq!(c.verified, 0, "the link to the draft is not counted");
    }

    #[test]
    fn references_to_nothing_are_errors_with_a_location() {
        let g = graph();
        let found = [
            note("REQ-NOPE-0001", Kind::Implements, "src/a.rs"),
            note("REQ-A-0001.S9", Kind::Verifies, "tests/a.rs"),
            note("other:REQ-X-0001", Kind::Implements, "src/b.rs"),
        ];
        let c = coverage(&g, &found, None);
        let codes: Vec<&str> = c.findings.iter().map(|f| f.code).collect();
        assert_eq!(codes, ["E014", "E015"], "{:?}", c.findings);
        assert_eq!(c.findings[0].path.as_deref(), Some(Path::new("src/a.rs")));
        assert_eq!(c.findings[0].line, Some(7));
        assert!(c.findings[0].message.contains("REQ-NOPE-0001"));
        assert!(c.findings[1].message.contains("S9"));
        assert_eq!(c.implemented, 0, "a wrong reference covers nothing");
    }

    #[test]
    fn scanning_a_tree_skips_markdown_ignored_excluded_and_binary_files() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        let write = |name: &str, text: &[u8]| {
            let path = root.join(name);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, text).unwrap();
        };
        write("src/a.rs", b"// spec: REQ-A-0001\n");
        write("tests/t.go", b"// verifies: REQ-A-0001.S1\n");
        write("docs/notes.md", b"// spec: REQ-A-0001\n");
        write("vendor/lib.rs", b"// spec: REQ-A-0001\n");
        write("blob.bin", b"\xff\xfe// spec: REQ-A-0001\n");
        write(".gitignore", b"ignored/\n");
        write("ignored/x.rs", b"// spec: REQ-A-0001\n");

        let scan = scan_tree(root, &["vendor/**".to_string()]).unwrap();
        let paths: Vec<String> = scan
            .annotations
            .iter()
            .map(|a| a.path.display().to_string())
            .collect();
        assert_eq!(paths, ["src/a.rs", "tests/t.go"]);
        assert_eq!(
            scan.files, 2,
            "markdown, vendor, binary and ignored files are not read"
        );

        assert!(scan_tree(root, &["[".to_string()]).is_err());
    }
}
