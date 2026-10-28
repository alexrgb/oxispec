//! Structural checks (layer 0): exact, fast, always on.
//!
//! E001 duplicate id       E002 dangling link        E003 missing id or type
//! E004 supersedes cycle   E005 invalid id format    E006 requirement without scenario
//! E007 requirement without SHALL or MUST            E008 requirement node without requirements
//! W001 unknown node type  I001 external reference, not checked

use std::collections::{HashMap, HashSet};

use crate::finding::{Finding, Severity};
use crate::graph::Graph;
use crate::id;
use crate::node::{KNOWN_TYPES, Node};

pub fn check(graph: &Graph) -> Vec<Finding> {
    let mut findings = graph.findings.clone();
    duplicates(graph, &mut findings);
    for node in &graph.nodes {
        node_format(node, &mut findings);
        links(graph, node, &mut findings);
        if node.ty == "requirement" {
            requirements(node, &mut findings);
        }
    }
    supersede_cycles(graph, &mut findings);
    findings
        .sort_by(|a, b| (a.path.as_ref(), a.line, a.code).cmp(&(b.path.as_ref(), b.line, b.code)));
    findings
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
        for target in targets {
            let Some(parsed) = id::parse(target) else {
                out.push(
                    Finding::error("E005", format!("invalid id {target:?} in `{kind}` link"))
                        .at(&node.path, Some(1))
                        .node(&node.id),
                );
                continue;
            };
            if parsed.namespace.is_some() {
                out.push(
                    Finding::info(
                        "I001",
                        format!("external reference {target} in `{kind}` link is not checked"),
                    )
                    .at(&node.path, Some(1))
                    .node(&node.id),
                );
            } else if graph.get(parsed.base).is_none() {
                out.push(
                    Finding::error(
                        "E002",
                        format!("dangling link: `{kind}` points to {target}, which does not exist"),
                    )
                    .at(&node.path, Some(1))
                    .node(&node.id),
                );
            }
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
}
