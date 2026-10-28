//! Structural checks (layer 0): exact, fast, always on.
//!
//! E001 duplicate id       E002 dangling link        E003 missing id or type
//! E004 supersedes cycle   E005 invalid id format    E006 requirement without scenario
//! E007 requirement without SHALL or MUST            E008 requirement node without requirements
//! E009 link to a scenario that does not exist       E010 duplicate scenario anchor
//! E011 invalid scenario anchor
//! E012 change set with a missing or unknown status
//! E013 approved or merged change set without `approved_by`
//! W001 unknown node type  W004 unknown link kind    I001 external reference, not checked

use std::collections::{HashMap, HashSet};

use crate::change;
use crate::finding::{Finding, Severity};
use crate::graph::Graph;
use crate::id;
use crate::node::{KNOWN_LINK_KINDS, KNOWN_TYPES, Node};

pub fn check(graph: &Graph) -> Vec<Finding> {
    let mut findings = graph.findings.clone();
    duplicates(graph, &mut findings);
    for node in &graph.nodes {
        node_format(node, &mut findings);
        links(graph, node, &mut findings);
        if node.ty == "requirement" {
            requirements(node, &mut findings);
        }
        if node.ty == "change" {
            change_set(node, &mut findings);
        }
    }
    supersede_cycles(graph, &mut findings);
    findings
        .sort_by(|a, b| (a.path.as_ref(), a.line, a.code).cmp(&(b.path.as_ref(), b.line, b.code)));
    findings
}

/// What a link target points at.
#[derive(Debug, PartialEq, Eq)]
pub enum LinkResolution {
    /// A node, or a scenario of a node, in this project.
    Ok,
    /// A namespaced ID that belongs to another project. Not checked here.
    External,
    /// Not a valid ID.
    Invalid,
    /// The node does not exist.
    MissingNode,
    /// The node exists but has no scenario with this anchor.
    MissingScenario(String),
}

pub fn resolve_link(graph: &Graph, target: &str) -> LinkResolution {
    let Some(parsed) = id::parse(target) else {
        return LinkResolution::Invalid;
    };
    if parsed.namespace.is_some() {
        return LinkResolution::External;
    }
    let Some(node) = graph.get(parsed.base) else {
        return LinkResolution::MissingNode;
    };
    match parsed.child {
        Some(child) if !node.has_scenario(child) => {
            LinkResolution::MissingScenario(child.to_string())
        }
        _ => LinkResolution::Ok,
    }
}

fn duplicates(graph: &Graph, out: &mut Vec<Finding>) {
    let mut seen: HashMap<&str, &Node> = HashMap::new();
    for n in &graph.nodes {
        if let Some(first) = seen.get(n.id.as_str()) {
            out.push(
                Finding::error(
                    "E001",
                    format!(
                        "duplicate id {} (first defined in {})",
                        n.id,
                        first.path.display()
                    ),
                )
                .at(&n.path, Some(1))
                .node(&n.id),
            );
        } else {
            seen.insert(&n.id, n);
        }
    }
}

fn node_format(node: &Node, out: &mut Vec<Finding>) {
    match id::parse(&node.id) {
        Some(p) if p.namespace.is_none() && p.child.is_none() => {}
        _ => out.push(
            Finding::error(
                "E005",
                format!(
                    "invalid id {:?}: use TYPE-NAME-0001 style, letters and digits between hyphens",
                    node.id
                ),
            )
            .at(&node.path, Some(1))
            .node(&node.id),
        ),
    }
    if !KNOWN_TYPES.contains(&node.ty.as_str()) {
        out.push(
            Finding::warning("W001", format!("unknown node type {:?}", node.ty))
                .at(&node.path, Some(1))
                .node(&node.id),
        );
    }
}

fn links(graph: &Graph, node: &Node, out: &mut Vec<Finding>) {
    for (kind, targets) in &node.links {
        if !KNOWN_LINK_KINDS.contains(&kind.as_str()) {
            out.push(
                Finding::warning(
                    "W004",
                    format!(
                        "unknown link kind `{kind}` (known: {})",
                        KNOWN_LINK_KINDS.join(", ")
                    ),
                )
                .at(&node.path, Some(1))
                .node(&node.id),
            );
        }
        for target in targets {
            let finding = match resolve_link(graph, target) {
                LinkResolution::Ok => continue,
                LinkResolution::Invalid => {
                    Finding::error("E005", format!("invalid id {target:?} in `{kind}` link"))
                }
                LinkResolution::External => Finding::info(
                    "I001",
                    format!("external reference {target} in `{kind}` link is not checked"),
                ),
                LinkResolution::MissingNode => Finding::error(
                    "E002",
                    format!("dangling link: `{kind}` points to {target}, which does not exist"),
                ),
                LinkResolution::MissingScenario(anchor) => Finding::error(
                    "E009",
                    format!(
                        "dangling link: `{kind}` points to {target}, but {} has no scenario with anchor {{#{anchor}}}",
                        id::base_of(target)
                    ),
                ),
            };
            out.push(finding.at(&node.path, Some(1)).node(&node.id));
        }
    }
}

fn requirements(node: &Node, out: &mut Vec<Finding>) {
    if node.requirements.is_empty() {
        out.push(
            Finding::error("E008", "requirement node has no `### Requirement:` block")
                .at(&node.path, Some(1))
                .node(&node.id),
        );
    }
    for r in &node.requirements {
        if r.scenarios.is_empty() {
            out.push(
                Finding::error(
                    "E006",
                    format!("requirement {:?} has no `#### Scenario:`", r.name),
                )
                .at(&node.path, Some(r.line))
                .node(&node.id),
            );
        }
        if !r.has_normative {
            out.push(
                Finding::error(
                    "E007",
                    format!(
                        "requirement {:?} needs SHALL or MUST in its description",
                        r.name
                    ),
                )
                .at(&node.path, Some(r.line))
                .node(&node.id),
            );
        }
    }
    scenario_anchors(node, out);
}

fn change_set(node: &Node, out: &mut Vec<Finding>) {
    let known = change::STATUSES.join(", ");
    match node.status.as_deref() {
        Some(status) if change::STATUSES.contains(&status) => {
            let approver = node.approved_by.as_deref().unwrap_or("").trim();
            if change::needs_approval(status) && approver.is_empty() {
                out.push(
                    Finding::error(
                        "E013",
                        format!("change set is {status} but has no `approved_by`"),
                    )
                    .at(&node.path, Some(1))
                    .node(&node.id),
                );
            }
        }
        Some(other) => out.push(
            Finding::error(
                "E012",
                format!("change set status {other:?} is not one of {known}"),
            )
            .at(&node.path, Some(1))
            .node(&node.id),
        ),
        None => out.push(
            Finding::error(
                "E012",
                format!("change set has no `status`; use one of {known}"),
            )
            .at(&node.path, Some(1))
            .node(&node.id),
        ),
    }
}

fn scenario_anchors(node: &Node, out: &mut Vec<Finding>) {
    let mut seen: HashMap<&str, usize> = HashMap::new();
    for scenario in node.requirements.iter().flat_map(|r| &r.scenarios) {
        let Some(anchor) = scenario.anchor.as_deref() else {
            continue;
        };
        if !id::is_valid_segment(anchor) {
            out.push(
                Finding::error(
                    "E011",
                    format!(
                        "invalid scenario anchor {{#{anchor}}}: use letters and digits only, for example {{#S1}}"
                    ),
                )
                .at(&node.path, Some(scenario.line))
                .node(&node.id),
            );
        } else if let Some(first) = seen.insert(anchor, scenario.line) {
            out.push(
                Finding::error(
                    "E010",
                    format!("scenario anchor {{#{anchor}}} is used twice (first on line {first})"),
                )
                .at(&node.path, Some(scenario.line))
                .node(&node.id),
            );
        }
    }
}

fn supersede_cycles(graph: &Graph, out: &mut Vec<Finding>) {
    fn visit<'a>(
        id: &'a str,
        index: &HashMap<&'a str, &'a Node>,
        done: &mut HashSet<&'a str>,
        stack: &mut Vec<&'a str>,
        cycles: &mut Vec<Vec<&'a str>>,
    ) {
        if done.contains(id) {
            return;
        }
        if let Some(pos) = stack.iter().position(|s| *s == id) {
            cycles.push(stack[pos..].to_vec());
            return;
        }
        stack.push(id);
        if let Some(node) = index.get(id) {
            for target in node.links.get("supersedes").into_iter().flatten() {
                visit(id::base_of(target), index, done, stack, cycles);
            }
        }
        stack.pop();
        done.insert(id);
    }

    let index: HashMap<&str, &Node> = graph.nodes.iter().map(|n| (n.id.as_str(), n)).collect();
    let mut done = HashSet::new();
    let mut cycles = Vec::new();
    for n in &graph.nodes {
        visit(&n.id, &index, &mut done, &mut Vec::new(), &mut cycles);
    }
    for cycle in cycles {
        let first = cycle[0];
        let path = index.get(first).map(|n| n.path.clone());
        let mut f = Finding::new(
            "E004",
            Severity::Error,
            format!("supersedes cycle: {} -> {}", cycle.join(" -> "), first),
        )
        .node(first);
        if let Some(p) = path {
            f = f.at(&p, Some(1));
        }
        out.push(f);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse::parse_file_text;
    use std::path::Path;

    fn graph(files: &[(&str, &str)]) -> Graph {
        let mut nodes = Vec::new();
        for (name, text) in files {
            let (node, findings) = parse_file_text(Path::new(name), text);
            assert!(findings.is_empty(), "{findings:?}");
            nodes.extend(node);
        }
        Graph::from_nodes(nodes)
    }

    fn codes(g: &Graph) -> Vec<&'static str> {
        check(g).iter().map(|f| f.code).collect()
    }

    const REQ: &str = "---\nid: REQ-A-0001\ntype: requirement\nlinks:\n  decided-by: ADR-0001\n---\n### Requirement: X\nIt SHALL work.\n\n#### Scenario: S\n- **WHEN** a\n- **THEN** b\n";
    const ADR: &str = "---\nid: ADR-0001\ntype: adr\n---\n# Decision\n";
    const REQ_ANCHORS: &str = "---\nid: REQ-A-0010\ntype: requirement\n---\n### Requirement: X\nIt SHALL work.\n\n#### Scenario: One {#S1}\n- **WHEN** a\n- **THEN** b\n\n#### Scenario: Two {#S2}\n- **WHEN** c\n- **THEN** d\n";

    fn adr_with_link(kind: &str, target: &str) -> String {
        format!("---\nid: ADR-0002\ntype: adr\nlinks:\n  {kind}: {target}\n---\n")
    }

    #[test]
    fn clean_graph_has_no_findings() {
        let g = graph(&[("r.md", REQ), ("a.md", ADR)]);
        assert!(codes(&g).is_empty());
    }

    #[test]
    fn dangling_link() {
        let g = graph(&[("r.md", REQ)]);
        assert_eq!(codes(&g), vec!["E002"]);
    }

    #[test]
    fn external_reference_is_info_only() {
        let text = REQ.replace("ADR-0001", "platform/identity:ADR-0009");
        let g = graph(&[("r.md", &text)]);
        assert_eq!(codes(&g), vec!["I001"]);
    }

    #[test]
    fn duplicate_ids() {
        let g = graph(&[("a.md", ADR), ("b.md", ADR)]);
        assert_eq!(codes(&g), vec!["E001"]);
    }

    #[test]
    fn requirement_rules() {
        let text = "---\nid: REQ-A-0002\ntype: requirement\n---\n### Requirement: X\nIt works.\n";
        let g = graph(&[("r.md", text)]);
        let mut c = codes(&g);
        c.sort();
        assert_eq!(c, vec!["E006", "E007"]);
        let empty = "---\nid: REQ-A-0003\ntype: requirement\n---\nNothing here.\n";
        assert_eq!(codes(&graph(&[("r.md", empty)])), vec!["E008"]);
    }

    #[test]
    fn supersede_cycle() {
        let a = "---\nid: ADR-0001\ntype: adr\nlinks:\n  supersedes: ADR-0002\n---\n";
        let b = "---\nid: ADR-0002\ntype: adr\nlinks:\n  supersedes: ADR-0001\n---\n";
        let g = graph(&[("a.md", a), ("b.md", b)]);
        assert_eq!(codes(&g), vec!["E004"]);
    }

    #[test]
    fn unknown_type_warns_and_bad_id_errors() {
        let t = "---\nid: BAD\ntype: gizmo\n---\n";
        let mut c = codes(&graph(&[("t.md", t)]));
        c.sort();
        assert_eq!(c, vec!["E005", "W001"]);
    }

    #[test]
    fn incoming_links_are_computed() {
        let g = graph(&[("r.md", REQ), ("a.md", ADR)]);
        let incoming = g.incoming("ADR-0001");
        assert_eq!(incoming.len(), 1);
        assert_eq!(incoming[0].0.id, "REQ-A-0001");
        assert_eq!(incoming[0].1, "decided-by");
    }

    #[test]
    fn link_to_an_existing_scenario_is_clean() {
        let adr = adr_with_link("refines", "REQ-A-0010.S2");
        let g = graph(&[("r.md", REQ_ANCHORS), ("a.md", &adr)]);
        assert!(codes(&g).is_empty());
    }

    #[test]
    fn link_to_a_missing_scenario_is_reported() {
        let adr = adr_with_link("refines", "REQ-A-0010.S9");
        let g = graph(&[("r.md", REQ_ANCHORS), ("a.md", &adr)]);
        assert_eq!(codes(&g), vec!["E009"]);
        let message = &check(&g)[0].message;
        assert!(message.contains("{#S9}"), "{message}");
    }

    #[test]
    fn scenario_link_to_a_node_without_scenarios_is_reported() {
        let target = "---\nid: ADR-0001\ntype: adr\n---\n";
        let source = adr_with_link("refines", "ADR-0001.S1");
        let g = graph(&[("a.md", target), ("b.md", &source)]);
        assert_eq!(codes(&g), vec!["E009"]);
    }

    #[test]
    fn duplicate_and_invalid_anchors() {
        let dup = REQ_ANCHORS.replace("{#S2}", "{#S1}");
        assert_eq!(codes(&graph(&[("r.md", &dup)])), vec!["E010"]);
        let bad = REQ_ANCHORS.replace("{#S2}", "{#bad-id}");
        assert_eq!(codes(&graph(&[("r.md", &bad)])), vec!["E011"]);
    }

    #[test]
    fn unknown_link_kind_warns_once_per_kind() {
        let adr = adr_with_link("frobnicates", "REQ-A-0010");
        let g = graph(&[("r.md", REQ_ANCHORS), ("a.md", &adr)]);
        assert_eq!(codes(&g), vec!["W004"]);
    }

    #[test]
    fn resolve_link_covers_every_case() {
        let g = graph(&[("r.md", REQ_ANCHORS)]);
        assert_eq!(resolve_link(&g, "REQ-A-0010"), LinkResolution::Ok);
        assert_eq!(resolve_link(&g, "REQ-A-0010.S1"), LinkResolution::Ok);
        assert_eq!(
            resolve_link(&g, "REQ-A-0010.S7"),
            LinkResolution::MissingScenario("S7".into())
        );
        assert_eq!(resolve_link(&g, "REQ-X-0001"), LinkResolution::MissingNode);
        assert_eq!(resolve_link(&g, "not an id"), LinkResolution::Invalid);
        assert_eq!(resolve_link(&g, "p/q:REQ-A-1"), LinkResolution::External);
    }

    fn change_set(status_lines: &str) -> String {
        format!(
            "---\nid: CHG-0001\ntype: change\n{status_lines}---\n# CHG-0001: Tighten timeouts\n"
        )
    }

    #[test]
    fn change_set_statuses_are_checked() {
        let ok = |lines: &str| codes(&graph(&[("c.md", &change_set(lines))]));
        assert!(ok("status: draft\n").is_empty());
        assert!(ok("status: in-review\n").is_empty());
        assert!(ok("status: reverted\n").is_empty());
        assert_eq!(ok(""), vec!["E012"]);
        assert_eq!(ok("status: finished\n"), vec!["E012"]);
    }

    #[test]
    fn approved_and_merged_change_sets_need_an_approver() {
        let ok = |lines: &str| codes(&graph(&[("c.md", &change_set(lines))]));
        assert_eq!(ok("status: approved\n"), vec!["E013"]);
        assert_eq!(ok("status: merged\napproved_by: \"  \"\n"), vec!["E013"]);
        assert!(
            ok("status: approved\napproved_by: Ann Lee\napproved_on: \"2026-10-28\"\n").is_empty()
        );
        assert!(ok("status: merged\napproved_by: Ann Lee\n").is_empty());
    }

    #[test]
    fn change_sets_can_link_to_what_they_change() {
        let c =
            "---\nid: CHG-0001\ntype: change\nstatus: draft\nlinks:\n  changes: REQ-A-0001\n---\n";
        let r = "---\nid: REQ-A-0001\ntype: requirement\n---\n### Requirement: X\nIt SHALL.\n\n#### Scenario: S\n";
        assert!(codes(&graph(&[("c.md", c), ("r.md", r)])).is_empty());
    }
}
