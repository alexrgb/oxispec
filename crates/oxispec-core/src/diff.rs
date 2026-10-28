//! Comparing nodes and graphs: what changed between two states of the specs.

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

use serde::Serialize;

use crate::check::check;
use crate::finding::{Finding, Severity};
use crate::graph::Graph;
use crate::node::{Node, RequirementBlock};

/// One difference between two versions of a node.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "change", rename_all = "snake_case")]
pub enum Change {
    Type {
        from: String,
        to: String,
    },
    Status {
        from: Option<String>,
        to: Option<String>,
    },
    Owner {
        from: Option<String>,
        to: Option<String>,
    },
    Moved {
        from: PathBuf,
        to: PathBuf,
    },
    LinkAdded {
        kind: String,
        target: String,
    },
    LinkRemoved {
        kind: String,
        target: String,
    },
    RequirementAdded {
        name: String,
    },
    RequirementRemoved {
        name: String,
    },
    ScenarioAdded {
        requirement: String,
        scenario: String,
    },
    ScenarioRemoved {
        requirement: String,
        scenario: String,
    },
    /// The text after the frontmatter is different.
    TextChanged,
}

impl Change {
    /// One line for people.
    pub fn describe(&self) -> String {
        fn opt(v: &Option<String>) -> &str {
            v.as_deref().unwrap_or("none")
        }
        match self {
            Change::Type { from, to } => format!("type: {from} -> {to}"),
            Change::Status { from, to } => format!("status: {} -> {}", opt(from), opt(to)),
            Change::Owner { from, to } => format!("owner: {} -> {}", opt(from), opt(to)),
            Change::Moved { from, to } => {
                format!("moved: {} -> {}", from.display(), to.display())
            }
            Change::LinkAdded { kind, target } => format!("link added: {kind} {target}"),
            Change::LinkRemoved { kind, target } => format!("link removed: {kind} {target}"),
            Change::RequirementAdded { name } => format!("requirement added: {name}"),
            Change::RequirementRemoved { name } => format!("requirement removed: {name}"),
            Change::ScenarioAdded {
                requirement,
                scenario,
            } => format!("scenario added: {scenario} (in {requirement})"),
            Change::ScenarioRemoved {
                requirement,
                scenario,
            } => format!("scenario removed: {scenario} (in {requirement})"),
            Change::TextChanged => "text changed".to_string(),
        }
    }
}

/// How a scenario is named in a change: its anchor if it has one, else its heading.
fn scenario_label(scenario: &crate::node::Scenario) -> String {
    match &scenario.anchor {
        Some(anchor) => format!("{{#{anchor}}}"),
        None => scenario.name.clone(),
    }
}

fn links(node: &Node) -> BTreeSet<(&str, &str)> {
    node.links
        .iter()
        .flat_map(|(kind, targets)| targets.iter().map(move |t| (kind.as_str(), t.as_str())))
        .collect()
}

fn requirements(node: &Node) -> BTreeMap<&str, &RequirementBlock> {
    node.requirements
        .iter()
        .map(|r| (r.name.as_str(), r))
        .collect()
}

/// What changed from `old` to `new`, in a fixed order. Empty when the two
/// are the same as far as OxiSpec can tell.
pub fn diff_nodes(old: &Node, new: &Node) -> Vec<Change> {
    let mut changes = Vec::new();
    if old.ty != new.ty {
        changes.push(Change::Type {
            from: old.ty.clone(),
            to: new.ty.clone(),
        });
    }
    if old.status != new.status {
        changes.push(Change::Status {
            from: old.status.clone(),
            to: new.status.clone(),
        });
    }
    if old.owner != new.owner {
        changes.push(Change::Owner {
            from: old.owner.clone(),
            to: new.owner.clone(),
        });
    }
    if old.path != new.path {
        changes.push(Change::Moved {
            from: old.path.clone(),
            to: new.path.clone(),
        });
    }

    let (old_links, new_links) = (links(old), links(new));
    for (kind, target) in new_links.difference(&old_links) {
        changes.push(Change::LinkAdded {
            kind: kind.to_string(),
            target: target.to_string(),
        });
    }
    for (kind, target) in old_links.difference(&new_links) {
        changes.push(Change::LinkRemoved {
            kind: kind.to_string(),
            target: target.to_string(),
        });
    }

    let (old_reqs, new_reqs) = (requirements(old), requirements(new));
    for name in new_reqs.keys().filter(|n| !old_reqs.contains_key(*n)) {
        changes.push(Change::RequirementAdded {
            name: name.to_string(),
        });
    }
    for name in old_reqs.keys().filter(|n| !new_reqs.contains_key(*n)) {
        changes.push(Change::RequirementRemoved {
            name: name.to_string(),
        });
    }
    for (name, new_req) in &new_reqs {
        let Some(old_req) = old_reqs.get(name) else {
            continue;
        };
        let old_labels: BTreeSet<String> = old_req.scenarios.iter().map(scenario_label).collect();
        let new_labels: BTreeSet<String> = new_req.scenarios.iter().map(scenario_label).collect();
        for label in new_labels.difference(&old_labels) {
            changes.push(Change::ScenarioAdded {
                requirement: name.to_string(),
                scenario: label.clone(),
            });
        }
        for label in old_labels.difference(&new_labels) {
            changes.push(Change::ScenarioRemoved {
                requirement: name.to_string(),
                scenario: label.clone(),
            });
        }
    }

    if old.body_hash != new.body_hash {
        changes.push(Change::TextChanged);
    }
    changes
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct NodeRef {
    pub id: String,
    #[serde(rename = "type")]
    pub ty: String,
    pub path: PathBuf,
}

impl From<&Node> for NodeRef {
    fn from(node: &Node) -> Self {
        NodeRef {
            id: node.id.clone(),
            ty: node.ty.clone(),
            path: node.path.clone(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct NodeChange {
    pub id: String,
    pub path: PathBuf,
    pub changes: Vec<Change>,
}

#[derive(Debug, Serialize)]
pub struct GraphDiff {
    pub added: Vec<NodeRef>,
    pub removed: Vec<NodeRef>,
    pub changed: Vec<NodeChange>,
    /// Errors and warnings that exist in the new graph but not the old one.
    pub new_findings: Vec<Finding>,
    /// Errors and warnings that existed in the old graph and are gone.
    pub fixed_findings: Vec<Finding>,
}

/// Findings are compared by what they say about which node, not by where
/// the node's file is, so moving a file does not make its findings new.
fn finding_key(f: &Finding) -> String {
    match &f.id {
        Some(id) => format!("{}|{id}|{}", f.code, f.message),
        None => format!(
            "{}||{}|{}",
            f.code,
            f.path
                .as_ref()
                .map(|p| p.display().to_string())
                .unwrap_or_default(),
            f.message
        ),
    }
}

fn reportable(f: &Finding) -> bool {
    f.severity != Severity::Info
}

/// What changed from `old` to `new`: nodes added, removed and changed, and
/// the errors and warnings that appeared or went away.
pub fn diff_graphs(old: &Graph, new: &Graph) -> GraphDiff {
    let mut added = Vec::new();
    let mut changed = Vec::new();
    for node in &new.nodes {
        // A duplicated ID is reported by the checks, so only its first node is compared.
        if !std::ptr::eq(new.get(&node.id).unwrap_or(node), node) {
            continue;
        }
        match old.get(&node.id) {
            None => added.push(NodeRef::from(node)),
            Some(before) => {
                let changes = diff_nodes(before, node);
                if !changes.is_empty() {
                    changed.push(NodeChange {
                        id: node.id.clone(),
                        path: node.path.clone(),
                        changes,
                    });
                }
            }
        }
    }
    let removed = old
        .nodes
        .iter()
        .filter(|n| std::ptr::eq(old.get(&n.id).unwrap_or(n), *n) && new.get(&n.id).is_none())
        .map(NodeRef::from)
        .collect();

    let (old_findings, new_findings) = (check(old), check(new));
    let old_keys: BTreeSet<String> = old_findings
        .iter()
        .filter(|f| reportable(f))
        .map(finding_key)
        .collect();
    let new_keys: BTreeSet<String> = new_findings
        .iter()
        .filter(|f| reportable(f))
        .map(finding_key)
        .collect();
    GraphDiff {
        added,
        removed,
        changed,
        new_findings: new_findings
            .into_iter()
            .filter(|f| reportable(f) && !old_keys.contains(&finding_key(f)))
            .collect(),
        fixed_findings: old_findings
            .into_iter()
            .filter(|f| reportable(f) && !new_keys.contains(&finding_key(f)))
            .collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse::parse_file_text;
    use std::path::Path;

    fn node(path: &str, text: &str) -> Node {
        parse_file_text(Path::new(path), text).0.expect("a node")
    }

    fn requirement(id: &str, extra_frontmatter: &str, body: &str) -> String {
        format!("---\nid: {id}\ntype: requirement\n{extra_frontmatter}---\n{body}")
    }

    const BODY: &str = "### Requirement: Login\nThe system SHALL issue a token.\n\n#### Scenario: Valid {#S1}\n- **WHEN** a\n- **THEN** b\n";

    #[test]
    fn identical_nodes_have_no_changes() {
        let a = node("a.md", &requirement("REQ-A-0001", "status: draft\n", BODY));
        assert!(diff_nodes(&a, &a.clone()).is_empty());
    }

    #[test]
    fn every_kind_of_change_is_reported_in_a_fixed_order() {
        let old = node(
            "a.md",
            &requirement(
                "REQ-A-0001",
                "status: draft\nlinks:\n  decided-by: ADR-0001\n",
                BODY,
            ),
        );
        let new_body = "### Requirement: Login\nThe system SHALL issue a token.\n\n#### Scenario: Valid {#S1}\n- **WHEN** a\n- **THEN** b\n\n#### Scenario: Expired {#S2}\n- **WHEN** c\n- **THEN** d\n\n### Requirement: Logout\nThe system SHALL end the session.\n";
        let new = node(
            "moved/a.md",
            &requirement(
                "REQ-A-0001",
                "status: accepted\nowner: team-a\nlinks:\n  decided-by: ADR-0002\n",
                new_body,
            ),
        );
        let described: Vec<String> = diff_nodes(&old, &new)
            .iter()
            .map(Change::describe)
            .collect();
        assert_eq!(
            described,
            vec![
                "status: draft -> accepted",
                "owner: none -> team-a",
                "moved: a.md -> moved/a.md",
                "link added: decided-by ADR-0002",
                "link removed: decided-by ADR-0001",
                "requirement added: Logout",
                "scenario added: {#S2} (in Login)",
                "text changed",
            ]
        );
    }

    #[test]
    fn removed_requirements_and_scenarios_are_reported() {
        let old = node("a.md", &requirement("REQ-A-0001", "", BODY));
        let new = node(
            "a.md",
            &requirement(
                "REQ-A-0001",
                "",
                "### Requirement: Login\nThe system SHALL issue a token.\n",
            ),
        );
        let described: Vec<String> = diff_nodes(&old, &new)
            .iter()
            .map(Change::describe)
            .collect();
        assert!(described.contains(&"scenario removed: {#S1} (in Login)".to_string()));
        let gone = node("a.md", &requirement("REQ-A-0001", "", "Nothing.\n"));
        let described: Vec<String> = diff_nodes(&old, &gone)
            .iter()
            .map(Change::describe)
            .collect();
        assert!(described.contains(&"requirement removed: Login".to_string()));
    }

    #[test]
    fn a_changed_comment_in_the_frontmatter_is_not_a_change() {
        let old = node("a.md", &requirement("REQ-A-0001", "status: draft\n", BODY));
        let new = node(
            "a.md",
            &requirement("REQ-A-0001", "# a note\nstatus: draft\n", BODY),
        );
        assert!(diff_nodes(&old, &new).is_empty());
    }

    fn graph(files: &[(&str, String)]) -> Graph {
        Graph::from_nodes(files.iter().map(|(p, t)| node(p, t)).collect())
    }

    #[test]
    fn graphs_report_added_removed_and_changed_nodes() {
        let old = graph(&[
            ("a.md", requirement("REQ-A-0001", "status: draft\n", BODY)),
            ("b.md", requirement("REQ-A-0002", "", BODY)),
        ]);
        let new = graph(&[
            (
                "a.md",
                requirement("REQ-A-0001", "status: accepted\n", BODY),
            ),
            ("c.md", requirement("REQ-A-0003", "", BODY)),
        ]);
        let diff = diff_graphs(&old, &new);
        assert_eq!(
            diff.added.iter().map(|n| n.id.as_str()).collect::<Vec<_>>(),
            ["REQ-A-0003"]
        );
        assert_eq!(
            diff.removed
                .iter()
                .map(|n| n.id.as_str())
                .collect::<Vec<_>>(),
            ["REQ-A-0002"]
        );
        assert_eq!(diff.changed.len(), 1);
        assert_eq!(diff.changed[0].id, "REQ-A-0001");
        assert_eq!(
            diff.changed[0].changes[0].describe(),
            "status: draft -> accepted"
        );
    }

    #[test]
    fn graphs_report_new_and_fixed_findings() {
        let broken = requirement("REQ-A-0001", "links:\n  refines: REQ-NOPE-0001\n", BODY);
        let fixed = requirement("REQ-A-0001", "", BODY);
        let old = graph(&[("a.md", fixed.clone())]);
        let new = graph(&[("a.md", broken.clone())]);

        let worse = diff_graphs(&old, &new);
        assert_eq!(worse.new_findings.len(), 1);
        assert_eq!(worse.new_findings[0].code, "E002");
        assert!(worse.fixed_findings.is_empty());

        let better = diff_graphs(&new, &old);
        assert!(better.new_findings.is_empty());
        assert_eq!(better.fixed_findings.len(), 1);
    }

    #[test]
    fn moving_a_file_does_not_make_its_findings_new() {
        let broken = requirement("REQ-A-0001", "links:\n  refines: REQ-NOPE-0001\n", BODY);
        let old = graph(&[("a.md", broken.clone())]);
        let new = graph(&[("moved/a.md", broken)]);
        let diff = diff_graphs(&old, &new);
        assert!(diff.new_findings.is_empty(), "{:?}", diff.new_findings);
        assert!(diff.fixed_findings.is_empty());
        assert_eq!(diff.changed.len(), 1);
        assert_eq!(
            diff.changed[0].changes[0].describe(),
            "moved: a.md -> moved/a.md"
        );
    }

    #[test]
    fn graph_from_sources_parses_files_that_are_not_on_disk() {
        let g = Graph::from_sources(vec![
            (
                PathBuf::from("a.md"),
                requirement("REQ-A-0001", "", BODY).into_bytes(),
            ),
            (PathBuf::from("bad.md"), vec![0xff, 0xfe, 0xfd]),
            (PathBuf::from("README.md"), b"# Not a node\n".to_vec()),
        ]);
        assert_eq!(g.nodes.len(), 1);
        assert_eq!(g.findings.len(), 1);
        assert_eq!(g.findings[0].code, "W003");
    }
}
