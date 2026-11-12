//! Test results, and what they say about scenarios.
//!
//! A scenario is *verified* when a test names it. It is *passing* when the
//! tests that name it ran and passed. This module reads results from JUnit
//! XML (what most test runners can write) and `go test -json`, and matches
//! them to the tests found by [`crate::trace`].

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use quick_xml::events::{BytesStart, Event};
use quick_xml::{Reader, XmlVersion};
use serde::{Deserialize, Serialize};

use crate::graph::Graph;
use crate::id;
use crate::trace::{Annotation, Kind, scan_text};

/// Where imported results are kept, relative to the project root. This is in
/// the cache folder: results belong to one run, not to the repository.
pub const RESULTS_PATH: &str = ".oxispec/cache/results.json";
const RESULTS_VERSION: u32 = 1;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("{0}: {1}")]
    Io(PathBuf, std::io::Error),
    #[error("cannot read test results from {0}: {1}")]
    Format(String, String),
    #[error("{0} is not a valid results file: {1}")]
    Stored(PathBuf, String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Outcome {
    Pass,
    Skip,
    Fail,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TestResult {
    /// The name the runner gave the test, for example `auth::idle` or
    /// `TestIdle/sub` or `ends idle sessions [REQ-AUTH-0012.S1]`.
    pub name: String,
    pub outcome: Outcome,
}

/// What `oxispec results import` keeps.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Results {
    pub version: u32,
    /// The commit that was checked out when the results were imported.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub commit: Option<String>,
    pub tests: Vec<TestResult>,
}

impl Default for Results {
    fn default() -> Self {
        Results {
            version: RESULTS_VERSION,
            commit: None,
            tests: Vec::new(),
        }
    }
}

impl Results {
    /// Adds results. A test seen again takes the newer outcome.
    pub fn merge(&mut self, tests: Vec<TestResult>) {
        let mut by_name: BTreeMap<String, Outcome> =
            self.tests.drain(..).map(|t| (t.name, t.outcome)).collect();
        for t in tests {
            by_name.insert(t.name, t.outcome);
        }
        self.tests = by_name
            .into_iter()
            .map(|(name, outcome)| TestResult { name, outcome })
            .collect();
    }

    pub fn count(&self, outcome: Outcome) -> usize {
        self.tests.iter().filter(|t| t.outcome == outcome).count()
    }
}

pub fn load(root: &Path) -> Result<Option<Results>, Error> {
    let path = root.join(RESULTS_PATH);
    let text = match fs::read_to_string(&path) {
        Ok(text) => text,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(Error::Io(path, e)),
    };
    let results: Results =
        serde_json::from_str(&text).map_err(|e| Error::Stored(path.clone(), e.to_string()))?;
    Ok((results.version == RESULTS_VERSION).then_some(results))
}

pub fn save(root: &Path, results: &Results) -> Result<(), Error> {
    let path = root.join(RESULTS_PATH);
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir).map_err(|e| Error::Io(dir.to_path_buf(), e))?;
    }
    let text =
        serde_json::to_string(results).map_err(|e| Error::Stored(path.clone(), e.to_string()))?;
    fs::write(&path, text).map_err(|e| Error::Io(path, e))
}

pub fn clear(root: &Path) -> Result<(), Error> {
    match fs::remove_file(root.join(RESULTS_PATH)) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(Error::Io(root.join(RESULTS_PATH), e)),
    }
}

/// Reads results from a file's text. `source` names it in error messages.
/// JUnit XML starts with `<`. `go test -json` is one JSON object per line.
pub fn parse(source: &str, text: &str) -> Result<Vec<TestResult>, Error> {
    let first = text.trim_start().chars().next();
    match first {
        Some('<') => parse_junit(source, text),
        Some('{') => parse_go_json(source, text),
        _ => Err(Error::Format(
            source.to_string(),
            "expected JUnit XML or `go test -json` output".to_string(),
        )),
    }
}

fn attribute(tag: &BytesStart, name: &str) -> Option<String> {
    tag.attributes()
        .flatten()
        .find(|a| a.key.as_ref() == name)
        .and_then(|a| a.normalized_value(XmlVersion::Implicit1_0).ok())
        .map(|v| v.into_owned())
}

fn parse_junit(source: &str, text: &str) -> Result<Vec<TestResult>, Error> {
    let fail = |e: &dyn std::fmt::Display| Error::Format(source.to_string(), e.to_string());
    let mut reader = Reader::from_str(text);
    let mut found: Vec<TestResult> = Vec::new();
    // The test case being read, and its outcome so far.
    let mut current: Option<(String, Outcome)> = None;
    loop {
        match reader.read_event().map_err(|e| fail(&e))? {
            Event::Start(tag) | Event::Empty(tag) => {
                let name = tag.name();
                match name.as_ref() {
                    "testcase" => {
                        if let Some((name, outcome)) = current.take() {
                            found.push(TestResult { name, outcome });
                        }
                        let name = attribute(&tag, "name").unwrap_or_default();
                        let class = attribute(&tag, "classname").filter(|c| !c.is_empty());
                        let full = match class {
                            Some(c) if !name.contains(&c) => format!("{c}::{name}"),
                            _ => name,
                        };
                        current = Some((full, Outcome::Pass));
                    }
                    "failure" | "error" => {
                        if let Some((_, outcome)) = current.as_mut() {
                            *outcome = Outcome::Fail;
                        }
                    }
                    "skipped" => {
                        if let Some((_, outcome)) = current.as_mut()
                            && *outcome == Outcome::Pass
                        {
                            *outcome = Outcome::Skip;
                        }
                    }
                    _ => {}
                }
            }
            Event::End(tag) if tag.name().as_ref() == "testcase" => {
                if let Some((name, outcome)) = current.take() {
                    found.push(TestResult { name, outcome });
                }
            }
            Event::Eof => break,
            _ => {}
        }
    }
    if let Some((name, outcome)) = current.take() {
        found.push(TestResult { name, outcome });
    }
    Ok(found)
}

#[derive(Deserialize)]
struct GoEvent {
    #[serde(rename = "Action")]
    action: String,
    #[serde(rename = "Package", default)]
    package: String,
    #[serde(rename = "Test", default)]
    test: Option<String>,
}

fn parse_go_json(source: &str, text: &str) -> Result<Vec<TestResult>, Error> {
    let mut outcomes: BTreeMap<String, Outcome> = BTreeMap::new();
    for (n, line) in text.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        let event: GoEvent = serde_json::from_str(line)
            .map_err(|e| Error::Format(source.to_string(), format!("line {}: {e}", n + 1)))?;
        let Some(test) = event.test else { continue };
        let outcome = match event.action.as_str() {
            "pass" => Outcome::Pass,
            "fail" => Outcome::Fail,
            "skip" => Outcome::Skip,
            _ => continue,
        };
        let name = if event.package.is_empty() {
            test
        } else {
            format!("{}.{test}", event.package)
        };
        outcomes.insert(name, outcome);
    }
    Ok(outcomes
        .into_iter()
        .map(|(name, outcome)| TestResult { name, outcome })
        .collect())
}

/// Whether the runner's name for a test belongs to a test found in the code.
///
/// `symbol` is the name tree-sitter gave the test: a function name such as
/// `idle`, or `it ends idle sessions` for a test call.
pub fn name_matches(result: &str, symbol: &str) -> bool {
    for call in ["it ", "test ", "describe "] {
        if let Some(title) = symbol.strip_prefix(call) {
            return !title.is_empty() && result.contains(title);
        }
    }
    if symbol.is_empty() || symbol.contains(' ') {
        return false;
    }
    // `crate::tests::idle`, `pkg.TestIdle`, `TestIdle/subtest`, `Class.method`.
    result.split(['/', ' ']).next().is_some_and(|head| {
        head == symbol
            || head.ends_with(&format!("::{symbol}"))
            || head.ends_with(&format!(".{symbol}"))
    })
}

/// How a scenario fared in the test runs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Status {
    Passing,
    Failing,
    /// Every matching test was skipped.
    Skipped,
    /// A test names the scenario, but no result matches it.
    NoResult,
}

/// Per scenario, from the annotations and the results. A scenario is a
/// node ID with its anchor, like `REQ-A-0001.S1`. A result also names a
/// scenario when its own name has the ID in brackets.
pub fn scenario_status(
    graph: &Graph,
    annotations: &[Annotation],
    results: &Results,
) -> BTreeMap<String, Status> {
    let annotations = &crate::trace::resolved(graph, annotations);
    // Outcomes of the results that belong to each scenario.
    let mut seen: BTreeMap<String, Vec<Outcome>> = BTreeMap::new();
    // Scenarios that some test names, with the test's symbol.
    let mut named: BTreeMap<String, Vec<Option<&str>>> = BTreeMap::new();

    let scenario_ids: BTreeSet<String> = graph
        .nodes
        .iter()
        .filter(|n| n.ty == "requirement")
        .flat_map(|n| {
            n.requirements
                .iter()
                .flat_map(|r| &r.scenarios)
                .filter_map(|s| s.anchor.as_ref())
                .map(|a| format!("{}.{a}", n.id))
        })
        .collect();
    // The scenarios of a requirement, for a `verifies` that names the whole.
    let of_node = |base: &str| -> Vec<&String> {
        scenario_ids
            .iter()
            .filter(|s| id::base_of(s) == base)
            .collect()
    };

    for a in annotations.iter().filter(|a| a.kind == Kind::Verifies) {
        let symbol = a.symbol.as_ref().map(|s| s.name.as_str());
        let targets: Vec<String> = match id::parse(&a.target) {
            Some(p) if p.child.is_some() => vec![a.target.clone()],
            Some(p) => of_node(p.base).into_iter().cloned().collect(),
            None => Vec::new(),
        };
        for target in targets {
            if scenario_ids.contains(&target) {
                named.entry(target).or_default().push(symbol);
            }
        }
    }

    for result in &results.tests {
        // Bracketed IDs in the result's own name.
        for annotation in scan_text(
            Path::new(""),
            &format!("// verifies: {}", brackets(&result.name)),
        ) {
            if scenario_ids.contains(&annotation.target) {
                seen.entry(annotation.target)
                    .or_default()
                    .push(result.outcome);
                continue;
            }
        }
        for (scenario, symbols) in &named {
            if symbols
                .iter()
                .flatten()
                .any(|symbol| name_matches(&result.name, symbol))
            {
                seen.entry(scenario.clone())
                    .or_default()
                    .push(result.outcome);
            }
        }
    }

    let mut status = BTreeMap::new();
    for scenario in named.keys().chain(seen.keys()) {
        let outcomes = seen.get(scenario).map(Vec::as_slice).unwrap_or_default();
        let s = if outcomes.is_empty() {
            Status::NoResult
        } else if outcomes.contains(&Outcome::Fail) {
            Status::Failing
        } else if outcomes.contains(&Outcome::Pass) {
            Status::Passing
        } else {
            Status::Skipped
        };
        status.insert(scenario.clone(), s);
    }
    status
}

/// The `[ID, ID]` groups of a test name, as a list `ID, ID` for the scanner.
fn brackets(name: &str) -> String {
    let mut out = Vec::new();
    let mut rest = name;
    while let Some(open) = rest.find('[') {
        rest = &rest[open + 1..];
        let Some(close) = rest.find(']') else { break };
        out.push(rest[..close].to_string());
        rest = &rest[close + 1..];
    }
    out.join(", ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse::parse_file_text;

    #[test]
    fn junit_xml_gives_pass_fail_and_skip() {
        let xml = r#"<?xml version="1.0"?>
<testsuites>
  <testsuite name="s" tests="5">
    <testcase classname="auth" name="idle" time="0.1"/>
    <testcase classname="auth" name="locked"><failure message="boom &amp; more">trace</failure></testcase>
    <testcase classname="auth" name="crashed"><error message="e"/></testcase>
    <testcase name="later"><skipped/></testcase>
    <testcase classname="a.b" name="a.b.same_prefix"/>
  </testsuite>
</testsuites>"#;
        let found = parse("junit.xml", xml).unwrap();
        let got: Vec<(&str, Outcome)> =
            found.iter().map(|t| (t.name.as_str(), t.outcome)).collect();
        assert_eq!(
            got,
            [
                ("auth::idle", Outcome::Pass),
                ("auth::locked", Outcome::Fail),
                ("auth::crashed", Outcome::Fail),
                ("later", Outcome::Skip),
                ("a.b.same_prefix", Outcome::Pass),
            ]
        );
        assert!(parse("bad.xml", "<testsuite><testcase name=\"x\"></testsuite>").is_err());
    }

    #[test]
    fn go_test_json_gives_one_result_per_test() {
        let json = "\
{\"Action\":\"run\",\"Package\":\"p\",\"Test\":\"TestIdle\"}
{\"Action\":\"output\",\"Package\":\"p\",\"Test\":\"TestIdle\",\"Output\":\"x\"}
{\"Action\":\"pass\",\"Package\":\"p\",\"Test\":\"TestIdle\"}
{\"Action\":\"fail\",\"Package\":\"p\",\"Test\":\"TestLocked/sub\"}
{\"Action\":\"skip\",\"Package\":\"p\",\"Test\":\"TestLater\"}
{\"Action\":\"pass\",\"Package\":\"p\"}
";
        let found = parse("go.json", json).unwrap();
        let got: Vec<(&str, Outcome)> =
            found.iter().map(|t| (t.name.as_str(), t.outcome)).collect();
        assert_eq!(
            got,
            [
                ("p.TestIdle", Outcome::Pass),
                ("p.TestLater", Outcome::Skip),
                ("p.TestLocked/sub", Outcome::Fail),
            ]
        );
        assert!(parse("go.json", "{not json}").is_err());
        assert!(parse("x.txt", "hello").is_err());
        assert!(parse("empty", "").is_err());
    }

    #[test]
    fn test_names_match_symbols_across_runners() {
        // Functions: Rust, Go, Python and Java style names.
        for result in [
            "idle",
            "auth::idle",
            "crate::auth::tests::idle",
            "p.idle",
            "Class.idle",
            "idle/sub",
        ] {
            assert!(name_matches(result, "idle"), "{result}");
        }
        for result in [
            "idle_later",
            "not_idle",
            "auth::idle2",
            "x::notidle",
            "idle extra words",
        ] {
            assert!(
                !name_matches(result, "idle") || result == "idle extra words",
                "{result}"
            );
        }
        assert!(
            name_matches("p.TestIdle/sub", "TestIdle") || name_matches("TestIdle/sub", "TestIdle")
        );
        // Test calls match on the title.
        assert!(name_matches(
            "auth ends idle sessions",
            "it ends idle sessions"
        ));
        assert!(!name_matches(
            "auth ends busy sessions",
            "it ends idle sessions"
        ));
        assert!(!name_matches("anything", "it "));
        assert!(!name_matches("anything", ""));
    }

    fn setup() -> (Graph, Vec<Annotation>) {
        let node = "---\nid: REQ-A-0001\ntype: requirement\n---\n### Requirement: A\nIt SHALL.\n#### Scenario: One {#S1}\n#### Scenario: Two {#S2}\n#### Scenario: Three {#S3}\n";
        let graph = Graph::from_nodes(
            parse_file_text(Path::new("a.md"), node)
                .0
                .into_iter()
                .collect(),
        );
        let code = "// verifies: REQ-A-0001.S1\n#[test]\nfn idle() {}\n\n// verifies: REQ-A-0001.S2\n#[test]\nfn locked() {}\n";
        (graph, scan_text(Path::new("tests/a.rs"), code))
    }

    fn results(items: &[(&str, Outcome)]) -> Results {
        let mut r = Results::default();
        r.merge(
            items
                .iter()
                .map(|(n, o)| TestResult {
                    name: n.to_string(),
                    outcome: *o,
                })
                .collect(),
        );
        r
    }

    #[test]
    fn a_scenario_passes_fails_or_has_no_result() {
        let (graph, found) = setup();
        let status = |items: &[(&str, Outcome)]| scenario_status(&graph, &found, &results(items));

        let s = status(&[("a::idle", Outcome::Pass), ("a::locked", Outcome::Fail)]);
        assert_eq!(s["REQ-A-0001.S1"], Status::Passing);
        assert_eq!(s["REQ-A-0001.S2"], Status::Failing);
        assert!(!s.contains_key("REQ-A-0001.S3"), "no test names it");

        let s = status(&[("a::idle", Outcome::Pass)]);
        assert_eq!(s["REQ-A-0001.S2"], Status::NoResult);
        let s = status(&[("a::idle", Outcome::Skip), ("a::locked", Outcome::Pass)]);
        assert_eq!(s["REQ-A-0001.S1"], Status::Skipped);
    }

    #[test]
    fn a_result_can_name_its_scenario_in_brackets() {
        let (graph, _) = setup();
        let s = scenario_status(
            &graph,
            &[],
            &results(&[
                ("ends idle sessions [REQ-A-0001.S3]", Outcome::Pass),
                ("two [REQ-A-0001.S1, REQ-A-0001.S2]", Outcome::Fail),
                ("mentions [REQ-NOPE-0001.S1]", Outcome::Pass),
            ]),
        );
        assert_eq!(s["REQ-A-0001.S3"], Status::Passing);
        assert_eq!(s["REQ-A-0001.S1"], Status::Failing);
        assert_eq!(s["REQ-A-0001.S2"], Status::Failing);
        assert_eq!(s.len(), 3);
    }

    #[test]
    fn a_failure_anywhere_wins_and_requirement_level_tests_cover_every_scenario() {
        let (graph, _) = setup();
        let found = scan_text(
            Path::new("t.go"),
            "// verifies: REQ-A-0001\nfunc TestAll(t *testing.T) {}\n",
        );
        let s = scenario_status(
            &graph,
            &found,
            &results(&[("p.TestAll", Outcome::Pass), ("TestAll/sub", Outcome::Fail)]),
        );
        assert_eq!(s.len(), 3);
        assert!(s.values().all(|v| *v == Status::Failing), "{s:?}");
    }

    #[test]
    fn merging_replaces_a_test_seen_again_and_counts_outcomes() {
        let mut r = results(&[("a", Outcome::Fail), ("b", Outcome::Pass)]);
        r.merge(vec![TestResult {
            name: "a".into(),
            outcome: Outcome::Pass,
        }]);
        assert_eq!(r.count(Outcome::Pass), 2);
        assert_eq!(r.count(Outcome::Fail), 0);
    }

    #[test]
    fn results_are_stored_in_the_cache_and_cleared() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        assert_eq!(load(root).unwrap(), None);
        let mut r = results(&[("a", Outcome::Pass)]);
        r.commit = Some("abc123".into());
        save(root, &r).unwrap();
        assert_eq!(load(root).unwrap(), Some(r));
        clear(root).unwrap();
        assert_eq!(load(root).unwrap(), None);
        clear(root).unwrap();
        fs::create_dir_all(root.join(".oxispec/cache")).unwrap();
        fs::write(root.join(RESULTS_PATH), "nope").unwrap();
        assert!(load(root).is_err());
    }
}
