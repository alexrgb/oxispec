//! Spec releases: what changed since the last one, which version that
//! deserves, the notes, and the gates a release must pass. This module only
//! decides. Reading git and writing files is done by the command line.

use std::cmp::Ordering;
use std::collections::BTreeSet;
use std::fmt;

use serde::Serialize;

use crate::diff::{Change, GraphDiff};
use crate::finding::Severity;
use crate::graph::Graph;
use crate::node::Node;

/// A semantic version, `MAJOR.MINOR.PATCH` with an optional `-pre.release`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Version {
    pub major: u64,
    pub minor: u64,
    pub patch: u64,
    pub pre: Option<String>,
}

impl Version {
    /// Accepts `1.2.3` and `1.2.3-rc.1`, with an optional leading `v`.
    pub fn parse(text: &str) -> Option<Version> {
        let text = text.strip_prefix('v').unwrap_or(text);
        let (core, pre) = match text.split_once('-') {
            Some((core, pre)) => (core, Some(pre)),
            None => (text, None),
        };
        if let Some(pre) = pre
            && (pre.is_empty()
                || !pre
                    .split('.')
                    .all(|p| !p.is_empty() && p.chars().all(|c| c.is_ascii_alphanumeric())))
        {
            return None;
        }
        let mut numbers = core.split('.').map(|p| {
            let plain = !p.is_empty()
                && p.chars().all(|c| c.is_ascii_digit())
                && (p == "0" || !p.starts_with('0'));
            plain.then(|| p.parse::<u64>().ok()).flatten()
        });
        let version = Version {
            major: numbers.next()??,
            minor: numbers.next()??,
            patch: numbers.next()??,
            pre: pre.map(str::to_string),
        };
        numbers.next().is_none().then_some(version)
    }

    fn core(&self) -> (u64, u64, u64) {
        (self.major, self.minor, self.patch)
    }
}

impl fmt::Display for Version {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)?;
        if let Some(pre) = &self.pre {
            write!(f, "-{pre}")?;
        }
        Ok(())
    }
}

impl PartialOrd for Version {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/// SemVer precedence: a pre-release sorts before its release, and numeric
/// pre-release parts sort before words.
impl Ord for Version {
    fn cmp(&self, other: &Self) -> Ordering {
        self.core()
            .cmp(&other.core())
            .then_with(|| match (&self.pre, &other.pre) {
                (None, None) => Ordering::Equal,
                (None, Some(_)) => Ordering::Greater,
                (Some(_), None) => Ordering::Less,
                (Some(a), Some(b)) => compare_pre(a, b),
            })
    }
}

fn compare_pre(a: &str, b: &str) -> Ordering {
    let mut left = a.split('.');
    let mut right = b.split('.');
    loop {
        match (left.next(), right.next()) {
            (None, None) => return Ordering::Equal,
            (None, Some(_)) => return Ordering::Less,
            (Some(_), None) => return Ordering::Greater,
            (Some(x), Some(y)) => {
                let order = match (x.parse::<u64>(), y.parse::<u64>()) {
                    (Ok(x), Ok(y)) => x.cmp(&y),
                    (Ok(_), Err(_)) => Ordering::Less,
                    (Err(_), Ok(_)) => Ordering::Greater,
                    (Err(_), Err(_)) => x.cmp(y),
                };
                if order != Ordering::Equal {
                    return order;
                }
            }
        }
    }
}

/// How big a change is, in the sense of docs/versioning.md.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Bump {
    None,
    Patch,
    Minor,
    Major,
}

/// Node types whose removal can break something that relies on them.
const CONTRACT_TYPES: &[&str] = &["requirement", "api", "state-machine"];

/// Process nodes. They record how the specs changed and are not part of
/// what a release says.
fn is_process(ty: &str) -> bool {
    matches!(ty, "change" | "release")
}

/// The changes between two states of the specs, sorted by how much they matter.
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize)]
pub struct Summary {
    /// Something that relies on the specs may stop working.
    pub breaking: Vec<String>,
    /// New requirements, scenarios or nodes.
    pub added: Vec<String>,
    /// Everything else: wording, links, status, moved files.
    pub changed: Vec<String>,
}

impl Summary {
    pub fn bump(&self) -> Bump {
        if !self.breaking.is_empty() {
            Bump::Major
        } else if !self.added.is_empty() {
            Bump::Minor
        } else if !self.changed.is_empty() {
            Bump::Patch
        } else {
            Bump::None
        }
    }

    pub fn is_empty(&self) -> bool {
        self.bump() == Bump::None
    }
}

/// The ID with a name, from the title or else the first requirement heading.
fn labelled(node: &Node) -> String {
    let name = node
        .title
        .as_deref()
        .or_else(|| node.requirements.first().map(|r| r.name.as_str()));
    match name {
        Some(name) => format!("{}: {name}", node.id),
        None => node.id.clone(),
    }
}

/// Sorts a diff into breaking, added and changed. `new` is the state being
/// released, used to look up titles.
pub fn summarize(new: &Graph, diff: &GraphDiff) -> Summary {
    let mut summary = Summary::default();
    for node in diff.added.iter().filter(|n| !is_process(&n.ty)) {
        let label = new.get(&node.id).map_or(node.id.clone(), labelled);
        summary.added.push(format!("{} {label}", node.ty));
    }
    for node in diff.removed.iter().filter(|n| !is_process(&n.ty)) {
        let line = format!("removed {} {}", node.ty, node.id);
        if CONTRACT_TYPES.contains(&node.ty.as_str()) {
            summary.breaking.push(line);
        } else {
            summary.changed.push(line);
        }
    }
    for node in &diff.changed {
        let Some(current) = new.get(&node.id) else {
            continue;
        };
        if is_process(&current.ty) {
            continue;
        }
        let label = labelled(current);
        let mut notes = Vec::new();
        for change in &node.changes {
            let line = format!("{}: {}", node.id, change.describe());
            match change {
                Change::RequirementRemoved { .. } | Change::ScenarioRemoved { .. } => {
                    summary.breaking.push(line);
                }
                Change::RequirementAdded { .. } | Change::ScenarioAdded { .. } => {
                    summary.added.push(line);
                }
                other => notes.push(other.describe()),
            }
        }
        if !notes.is_empty() {
            summary
                .changed
                .push(format!("{label} ({})", notes.join("; ")));
        }
    }
    summary
}

/// The version a release should have, from the previous one and the size of
/// the changes. Before 1.0 a breaking change bumps the minor version, as in
/// docs/versioning.md. `None` when nothing changed since a previous release.
pub fn suggest(base: Option<&Version>, bump: Bump) -> Option<Version> {
    let Some(base) = base else {
        return Some(Version {
            major: 0,
            minor: 1,
            patch: 0,
            pre: None,
        });
    };
    let (major, minor, patch) = match (bump, base.major) {
        (Bump::None, _) => return None,
        (Bump::Major, 0) | (Bump::Minor, 0) => (0, base.minor + 1, 0),
        (Bump::Patch, 0) => (0, base.minor, base.patch + 1),
        (Bump::Major, _) => (base.major + 1, 0, 0),
        (Bump::Minor, _) => (base.major, base.minor + 1, 0),
        (Bump::Patch, _) => (base.major, base.minor, base.patch + 1),
    };
    Some(Version {
        major,
        minor,
        patch,
        pre: None,
    })
}

/// The change sets a release covers: those added or edited since the base.
pub fn included_change_sets<'a>(new: &'a Graph, diff: &GraphDiff) -> Vec<&'a Node> {
    let ids: BTreeSet<&str> = diff
        .added
        .iter()
        .map(|n| n.id.as_str())
        .chain(diff.changed.iter().map(|n| n.id.as_str()))
        .collect();
    new.nodes
        .iter()
        .filter(|n| n.ty == "change" && ids.contains(n.id.as_str()))
        .collect()
}

/// The release notes: the body of the manifest after its heading.
pub fn notes(base: Option<&Version>, summary: &Summary, change_sets: &[&Node]) -> String {
    let mut out = String::new();
    match base {
        Some(base) => out.push_str(&format!("Changes since {base}.\n")),
        None => out.push_str("The first release.\n"),
    }
    let mut section = |title: &str, items: &[String]| {
        if !items.is_empty() {
            out.push_str(&format!("\n## {title}\n\n"));
            for item in items {
                out.push_str(&format!("- {item}\n"));
            }
        }
    };
    section("Breaking", &summary.breaking);
    section("Added", &summary.added);
    section("Changed", &summary.changed);
    let sets: Vec<String> = change_sets.iter().map(|n| labelled(n)).collect();
    section("Change sets", &sets);
    out
}

/// One gate a release must pass.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Gate {
    pub name: &'static str,
    pub passed: bool,
    /// A gate that only warns does not stop a release.
    pub blocking: bool,
    pub detail: String,
}

impl Gate {
    fn new(name: &'static str, passed: bool, detail: impl Into<String>) -> Gate {
        Gate {
            name,
            passed,
            blocking: true,
            detail: detail.into(),
        }
    }

    fn advisory(mut self) -> Gate {
        self.blocking = false;
        self
    }
}

/// What the code and the tests say, for the gates. Only given for a project
/// that uses trace annotations.
pub struct TraceFacts {
    /// Links between specs and code that changed since they were confirmed.
    pub suspect: usize,
    /// Scenarios whose tests failed in the imported results.
    pub failing: Vec<String>,
    /// What was imported. `None` when no results were imported.
    pub results: Option<ResultsFacts>,
}

pub struct ResultsFacts {
    /// The results are from the commit that is being released.
    pub current: bool,
}

/// Everything the gates look at.
pub struct Candidate<'a> {
    pub version: &'a Version,
    pub base: Option<&'a Version>,
    pub manifest: &'a Node,
    pub graph: &'a Graph,
    pub diff: &'a GraphDiff,
    /// A tag for this version already exists.
    pub tag_exists: bool,
    pub trace: Option<TraceFacts>,
}

/// Runs the gates. The release can go ahead when no blocking gate failed.
pub fn gates(candidate: &Candidate) -> Vec<Gate> {
    let Candidate {
        version,
        base,
        manifest,
        graph,
        diff,
        tag_exists,
        trace,
    } = candidate;
    let summary = summarize(graph, diff);
    let included = included_change_sets(graph, diff);
    let mut gates = Vec::new();

    gates.push(Gate::new(
        "not-released",
        !tag_exists,
        if *tag_exists {
            format!("{version} is already released; tags are never moved")
        } else {
            format!("{version} has not been released")
        },
    ));

    let version_ok = base.is_none_or(|b| version.core() > b.core());
    gates.push(Gate::new(
        "version-increases",
        version_ok,
        match base {
            Some(b) if !version_ok => format!("{version} is not newer than the last release, {b}"),
            Some(b) => format!("{version} is newer than {b}"),
            None => "there is no earlier release".to_string(),
        },
    ));

    gates.push(Gate::new(
        "has-changes",
        base.is_none() || !summary.is_empty(),
        if base.is_some() && summary.is_empty() {
            "nothing has changed since the last release".to_string()
        } else {
            "the specs changed since the last release".to_string()
        },
    ));

    let errors: Vec<_> = crate::check(graph)
        .into_iter()
        .filter(|f| f.severity == Severity::Error)
        .collect();
    gates.push(Gate::new(
        "no-errors",
        errors.is_empty(),
        if errors.is_empty() {
            "oxispec check reports no errors".to_string()
        } else {
            format!("oxispec check reports {} error(s)", errors.len())
        },
    ));

    let open: Vec<&str> = included
        .iter()
        .filter(|n| !matches!(n.status.as_deref(), Some("approved" | "merged")))
        .map(|n| n.id.as_str())
        .collect();
    gates.push(Gate::new(
        "change-sets-approved",
        open.is_empty(),
        if open.is_empty() {
            format!("{} change set(s), all approved or merged", included.len())
        } else {
            format!("not approved yet: {}", open.join(", "))
        },
    ));

    let listed: BTreeSet<&str> = manifest
        .links
        .get("includes")
        .into_iter()
        .flatten()
        .map(String::as_str)
        .collect();
    let current: BTreeSet<&str> = included.iter().map(|n| n.id.as_str()).collect();
    gates.push(Gate::new(
        "manifest-current",
        listed == current,
        if listed == current {
            "the manifest lists the change sets since the last release".to_string()
        } else {
            "the manifest is out of date; run `oxispec release prepare --force`".to_string()
        },
    ));

    if let Some(facts) = trace {
        gates.push(Gate::new(
            "no-suspect-links",
            facts.suspect == 0,
            if facts.suspect == 0 {
                "no link between a spec and code is suspect".to_string()
            } else {
                format!(
                    "{} link(s) between specs and code changed since they were confirmed; run `oxispec suspect`",
                    facts.suspect
                )
            },
        ));
        gates.push(match &facts.results {
            None => Gate::new(
                "tests-passing",
                false,
                "no test results were imported; run `oxispec results import`",
            )
            .advisory(),
            Some(_) => Gate::new(
                "tests-passing",
                facts.failing.is_empty(),
                if facts.failing.is_empty() {
                    "no scenario has a failing test".to_string()
                } else {
                    format!("failing: {}", facts.failing.join(", "))
                },
            ),
        });
        if let Some(results) = &facts.results {
            gates.push(
                Gate::new(
                    "results-current",
                    results.current,
                    if results.current {
                        "the test results are from this commit".to_string()
                    } else {
                        "the test results are from another commit; run the tests and import again"
                            .to_string()
                    },
                )
                .advisory(),
            );
        }
    }

    if !graph.debt.limits.is_empty() {
        let over = crate::debt::over_limits(graph);
        gates.push(Gate::new(
            "debt-within-limits",
            over.is_empty(),
            if over.is_empty() {
                "the tech debt that is still to be paid is within the limits".to_string()
            } else {
                over.iter()
                    .map(|(severity, live, limit)| {
                        format!("{live} {severity} debt items, and the limit is {limit}")
                    })
                    .collect::<Vec<_>>()
                    .join("; ")
            },
        ));
    }

    let (rules, problems) = crate::compliance::resolve(graph.compliance_config());
    if !rules.is_empty() || !problems.is_empty() {
        let gaps: usize = crate::compliance::report(graph)
            .iter()
            .map(|r| r.gaps.len())
            .sum();
        gates.push(Gate::new(
            "compliance-met",
            gaps == 0 && problems.is_empty(),
            if !problems.is_empty() {
                format!(
                    "the compliance settings are not valid: {}",
                    problems.join("; ")
                )
            } else if gaps == 0 {
                format!("the {} compliance rules are met", rules.len())
            } else {
                format!("{gaps} gaps in traceability or sign-offs; see `oxispec compliance`")
            },
        ));
    }

    if let Some(suggested) = suggest(*base, summary.bump()) {
        let enough = version.core() >= suggested.core();
        gates.push(
            Gate::new(
                "version-matches-changes",
                enough,
                if enough {
                    format!("{version} is at least the suggested {suggested}")
                } else {
                    format!(
                        "the changes suggest {suggested} (a {:?} bump), and {version} is lower",
                        summary.bump()
                    )
                },
            )
            .advisory(),
        );
    }
    gates
}

/// True when no blocking gate failed.
pub fn can_release(gates: &[Gate]) -> bool {
    gates.iter().all(|g| g.passed || !g.blocking)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diff::diff_graphs;
    use crate::parse::parse_file_text;
    use std::path::Path;

    fn v(text: &str) -> Version {
        Version::parse(text).unwrap_or_else(|| panic!("{text}"))
    }

    const REQ: &str = "### Requirement: X\nThe system SHALL work.\n\n#### Scenario: S {#S1}\n- **WHEN** a\n- **THEN** b\n";

    fn graph(files: &[(&str, String)]) -> Graph {
        Graph::from_nodes(
            files
                .iter()
                .filter_map(|(p, t)| parse_file_text(Path::new(p), t).0)
                .collect(),
        )
    }

    fn req(id: &str, extra: &str, body: &str) -> String {
        format!("---\nid: {id}\ntype: requirement\n{extra}---\n{body}")
    }

    fn change(id: &str, status: &str) -> String {
        let approval = if matches!(status, "approved" | "merged") {
            "approved_by: Ann Lee\napproved_on: \"2026-10-28\"\n"
        } else {
            ""
        };
        format!("---\nid: {id}\ntype: change\nstatus: {status}\n{approval}---\n# {id}: Do it\n")
    }

    #[test]
    fn versions_parse_and_print() {
        for good in ["0.1.0", "1.2.3", "10.20.30", "1.0.0-rc.1", "v2.0.0"] {
            let parsed = v(good);
            assert_eq!(parsed.to_string(), good.trim_start_matches('v'));
        }
        for bad in [
            "",
            "1",
            "1.2",
            "1.2.3.4",
            "01.2.3",
            "1.2.x",
            "1.2.3-",
            "1.2.3-a..b",
            "-1.2.3",
            "a.b.c",
        ] {
            assert!(Version::parse(bad).is_none(), "{bad}");
        }
    }

    #[test]
    fn versions_sort_by_semver_precedence() {
        let ordered = [
            "0.9.0",
            "1.0.0-alpha",
            "1.0.0-alpha.1",
            "1.0.0-alpha.beta",
            "1.0.0-beta",
            "1.0.0-beta.2",
            "1.0.0-beta.11",
            "1.0.0-rc.1",
            "1.0.0",
            "1.0.1",
            "1.10.0",
            "2.0.0",
        ];
        for pair in ordered.windows(2) {
            assert!(v(pair[0]) < v(pair[1]), "{} < {}", pair[0], pair[1]);
        }
        assert_eq!(v("1.2.3"), v("v1.2.3"));
    }

    #[test]
    fn a_breaking_change_bumps_the_minor_version_before_one_point_zero() {
        let base = v("0.3.1");
        assert_eq!(suggest(Some(&base), Bump::Major), Some(v("0.4.0")));
        assert_eq!(suggest(Some(&base), Bump::Minor), Some(v("0.4.0")));
        assert_eq!(suggest(Some(&base), Bump::Patch), Some(v("0.3.2")));
        assert_eq!(suggest(Some(&base), Bump::None), None);
    }

    #[test]
    fn from_one_point_zero_the_bump_follows_semver() {
        let base = v("1.4.2");
        assert_eq!(suggest(Some(&base), Bump::Major), Some(v("2.0.0")));
        assert_eq!(suggest(Some(&base), Bump::Minor), Some(v("1.5.0")));
        assert_eq!(suggest(Some(&base), Bump::Patch), Some(v("1.4.3")));
        assert_eq!(suggest(None, Bump::Patch), Some(v("0.1.0")));
    }

    #[test]
    fn changes_are_sorted_into_the_right_bump() {
        let old = graph(&[
            ("a.md", req("REQ-A-0001", "", REQ)),
            ("b.md", req("REQ-A-0002", "", REQ)),
        ]);

        // Wording only is a patch.
        let reworded = graph(&[
            (
                "a.md",
                req("REQ-A-0001", "", &REQ.replace("work", "work well")),
            ),
            ("b.md", req("REQ-A-0002", "", REQ)),
        ]);
        let summary = summarize(&reworded, &diff_graphs(&old, &reworded));
        assert_eq!(summary.bump(), Bump::Patch);
        assert!(summary.changed[0].contains("text changed"));

        // A new requirement is a minor change.
        let more = graph(&[
            ("a.md", req("REQ-A-0001", "", REQ)),
            ("b.md", req("REQ-A-0002", "", REQ)),
            ("c.md", req("REQ-A-0003", "", REQ)),
        ]);
        assert_eq!(
            summarize(&more, &diff_graphs(&old, &more)).bump(),
            Bump::Minor
        );

        // A removed requirement is breaking.
        let fewer = graph(&[("a.md", req("REQ-A-0001", "", REQ))]);
        let summary = summarize(&fewer, &diff_graphs(&old, &fewer));
        assert_eq!(summary.bump(), Bump::Major);
        assert_eq!(summary.breaking, ["removed requirement REQ-A-0002"]);

        // A removed scenario is breaking, an added one is not.
        let two = format!("{REQ}\n#### Scenario: T {{#S2}}\n- **WHEN** c\n- **THEN** d\n");
        let wider = graph(&[("a.md", req("REQ-A-0001", "", &two))]);
        let narrower = graph(&[("a.md", req("REQ-A-0001", "", REQ))]);
        assert_eq!(
            summarize(&wider, &diff_graphs(&narrower, &wider)).bump(),
            Bump::Minor
        );
        let summary = summarize(&narrower, &diff_graphs(&wider, &narrower));
        assert_eq!(summary.bump(), Bump::Major);
        assert!(summary.breaking[0].contains("scenario removed"));

        // Nothing changed.
        assert_eq!(summarize(&old, &diff_graphs(&old, &old)).bump(), Bump::None);
    }

    #[test]
    fn change_sets_and_releases_do_not_count_as_changes() {
        let old = graph(&[("a.md", req("REQ-A-0001", "", REQ))]);
        let new = graph(&[
            ("a.md", req("REQ-A-0001", "", REQ)),
            ("c.md", change("CHG-0001", "draft")),
            (
                "r.md",
                "---\nid: REL-0001\ntype: release\nversion: \"0.1.0\"\n---\n".to_string(),
            ),
        ]);
        let diff = diff_graphs(&old, &new);
        assert!(summarize(&new, &diff).is_empty());
        assert_eq!(included_change_sets(&new, &diff)[0].id, "CHG-0001");
    }

    #[test]
    fn notes_list_sections_that_have_content() {
        let summary = Summary {
            breaking: vec!["removed requirement REQ-A-0002".into()],
            added: vec![],
            changed: vec!["REQ-A-0001: Login (text changed)".into()],
        };
        let new = graph(&[("c.md", change("CHG-0001", "approved"))]);
        let sets: Vec<&Node> = new.nodes.iter().collect();
        let text = notes(Some(&v("0.1.0")), &summary, &sets);
        assert!(text.starts_with("Changes since 0.1.0.\n"));
        assert!(text.contains("## Breaking\n\n- removed requirement REQ-A-0002\n"));
        assert!(text.contains("## Changed\n"));
        assert!(!text.contains("## Added"));
        assert!(text.contains("## Change sets\n\n- CHG-0001: Do it\n"));
        assert!(notes(None, &Summary::default(), &[]).starts_with("The first release.\n"));
    }

    fn gate<'a>(gates: &'a [Gate], name: &str) -> &'a Gate {
        gates.iter().find(|g| g.name == name).unwrap()
    }

    fn manifest(changes: &str) -> Node {
        let text = format!(
            "---\nid: REL-0001\ntype: release\nstatus: candidate\nversion: \"0.2.0\"\nlinks:\n  includes: [{changes}]\n---\n"
        );
        parse_file_text(Path::new("r.md"), &text).0.unwrap()
    }

    #[test]
    fn gates_pass_for_a_clean_release_and_fail_for_each_problem() {
        let old = graph(&[("a.md", req("REQ-A-0001", "", REQ))]);
        let new = graph(&[
            (
                "a.md",
                req("REQ-A-0001", "", &REQ.replace("work", "work well")),
            ),
            ("c.md", change("CHG-0001", "approved")),
        ]);
        let diff = diff_graphs(&old, &new);
        let base = v("0.1.0");
        let target = v("0.1.1");
        let ready = manifest("CHG-0001");
        let candidate = |version, manifest, tag_exists| Candidate {
            version,
            base: Some(&base),
            manifest,
            graph: &new,
            diff: &diff,
            tag_exists,
            trace: None,
        };

        let all = gates(&candidate(&target, &ready, false));
        assert!(can_release(&all), "{all:?}");
        assert!(all.iter().all(|g| g.passed), "{all:?}");

        // Already released.
        let all = gates(&candidate(&target, &ready, true));
        assert!(!gate(&all, "not-released").passed);
        assert!(!can_release(&all));

        // Not newer than the base.
        let all = gates(&candidate(&base, &ready, false));
        assert!(!gate(&all, "version-increases").passed);

        // A stale manifest.
        let stale = manifest("");
        let all = gates(&candidate(&target, &stale, false));
        assert!(!gate(&all, "manifest-current").passed);

        // Too small a version only warns.
        let removed = graph(&[("c.md", change("CHG-0001", "approved"))]);
        let diff_removed = diff_graphs(&old, &removed);
        let all = gates(&Candidate {
            version: &target,
            base: Some(&base),
            manifest: &ready,
            graph: &removed,
            diff: &diff_removed,
            tag_exists: false,
            trace: None,
        });
        let warning = gate(&all, "version-matches-changes");
        assert!(!warning.passed && !warning.blocking, "{warning:?}");
        assert!(can_release(&all), "{all:?}");
    }

    #[test]
    fn unapproved_change_sets_and_errors_block_a_release() {
        let old = graph(&[("a.md", req("REQ-A-0001", "", REQ))]);
        let base = v("0.1.0");
        let target = v("0.2.0");

        let in_review = graph(&[
            ("a.md", req("REQ-A-0001", "", REQ)),
            ("b.md", req("REQ-A-0002", "", REQ)),
            ("c.md", change("CHG-0001", "in-review")),
        ]);
        let diff = diff_graphs(&old, &in_review);
        let ready = manifest("CHG-0001");
        let all = gates(&Candidate {
            version: &target,
            base: Some(&base),
            manifest: &ready,
            graph: &in_review,
            diff: &diff,
            tag_exists: false,
            trace: None,
        });
        assert_eq!(
            gate(&all, "change-sets-approved").detail,
            "not approved yet: CHG-0001"
        );
        assert!(!can_release(&all));

        // A dangling link is an error.
        let broken = graph(&[
            (
                "a.md",
                req("REQ-A-0001", "links:\n  refines: REQ-NOPE-0001\n", REQ),
            ),
            ("c.md", change("CHG-0001", "approved")),
        ]);
        let diff = diff_graphs(&old, &broken);
        let all = gates(&Candidate {
            version: &target,
            base: Some(&base),
            manifest: &ready,
            graph: &broken,
            diff: &diff,
            tag_exists: false,
            trace: None,
        });
        assert!(!gate(&all, "no-errors").passed);
        assert!(!can_release(&all));

        // No changes at all since the last release.
        let same = diff_graphs(&old, &old);
        let none = manifest("");
        let all = gates(&Candidate {
            version: &target,
            base: Some(&base),
            manifest: &none,
            graph: &old,
            diff: &same,
            tag_exists: false,
            trace: None,
        });
        assert!(!gate(&all, "has-changes").passed);
    }

    #[test]
    fn suspect_links_and_failing_tests_block_a_release() {
        let new = graph(&[("a.md", req("REQ-A-0001", "", REQ))]);
        let diff = diff_graphs(&Graph::default(), &new);
        let target = v("0.1.0");
        let ready = manifest("");
        let run = |trace| {
            gates(&Candidate {
                version: &target,
                base: None,
                manifest: &ready,
                graph: &new,
                diff: &diff,
                tag_exists: false,
                trace,
            })
        };

        // A project that does not use trace has none of these gates.
        assert!(!run(None).iter().any(|g| g.name == "no-suspect-links"));

        let clean = run(Some(TraceFacts {
            suspect: 0,
            failing: vec![],
            results: Some(ResultsFacts { current: true }),
        }));
        assert!(can_release(&clean), "{clean:?}");
        assert!(clean.iter().all(|g| g.passed), "{clean:?}");

        let suspect = run(Some(TraceFacts {
            suspect: 2,
            failing: vec![],
            results: Some(ResultsFacts { current: true }),
        }));
        assert!(!gate(&suspect, "no-suspect-links").passed);
        assert!(!can_release(&suspect));

        let failing = run(Some(TraceFacts {
            suspect: 0,
            failing: vec!["REQ-A-0001.S1".into()],
            results: Some(ResultsFacts { current: true }),
        }));
        assert_eq!(
            gate(&failing, "tests-passing").detail,
            "failing: REQ-A-0001.S1"
        );
        assert!(!can_release(&failing));

        // No results, or old ones, only warn.
        let none = run(Some(TraceFacts {
            suspect: 0,
            failing: vec![],
            results: None,
        }));
        assert!(!gate(&none, "tests-passing").passed && !gate(&none, "tests-passing").blocking);
        assert!(can_release(&none), "{none:?}");
        let old = run(Some(TraceFacts {
            suspect: 0,
            failing: vec![],
            results: Some(ResultsFacts { current: false }),
        }));
        assert!(!gate(&old, "results-current").passed && !gate(&old, "results-current").blocking);
        assert!(can_release(&old), "{old:?}");
    }

    #[test]
    fn the_first_release_needs_no_changes_or_base() {
        let new = graph(&[("a.md", req("REQ-A-0001", "", REQ))]);
        let diff = diff_graphs(&Graph::default(), &new);
        let target = v("0.1.0");
        let ready = manifest("");
        let all = gates(&Candidate {
            version: &target,
            base: None,
            manifest: &ready,
            graph: &new,
            diff: &diff,
            tag_exists: false,
            trace: None,
        });
        assert!(can_release(&all), "{all:?}");
    }
}
