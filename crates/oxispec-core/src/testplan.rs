//! Test plans: what is to be tested, and whether it is.
//!
//! A `test-plan` node says what a plan covers with `covers` links to
//! requirements and risks, and keeps its scope, approach, levels,
//! environments and exit criteria as text. It is not a test: the tests are
//! tied to scenarios as always, and the plan is read against that. A plan
//! uses `covers` and not `verifies`, so it never counts as a test.
//!
//! E040 the status of a plan is missing or not valid, or a date is not one
//! W020 a plan covers nothing
//! W021 a requirement that has been accepted is covered by no approved plan
//! W022 a live risk at the high or critical level is covered by no approved plan
//!
//! W021 and W022 are raised only when the project has an approved plan, so a
//! project that does not use plans sees neither.

use std::collections::BTreeSet;

use crate::debt::day_number;
use crate::finding::Finding;
use crate::graph::Graph;
use crate::id;
use crate::node::Node;
use crate::risks;

pub const STATUSES: &[&str] = &["draft", "approved", "retired"];

/// A requirement a plan covers, and how its scenarios stand.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RequirementRow {
    pub id: String,
    /// Scenarios that have an anchor.
    pub scenarios: usize,
    /// Scenarios no test names, like `REQ-A-0001.S2`.
    pub unverified: Vec<String>,
    /// Scenarios whose imported results fail.
    pub failing: Vec<String>,
}

/// A risk a plan covers, and whether what mitigates it is tested.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RiskRow {
    pub id: String,
    pub level: Option<&'static str>,
    pub live: bool,
    /// The requirements named by `mitigated-by`.
    pub mitigations: Vec<String>,
    /// True when it has mitigations and every scenario of them is verified.
    pub tested: bool,
}

#[derive(Debug)]
pub struct PlanReport<'a> {
    pub node: &'a Node,
    pub requirements: Vec<RequirementRow>,
    pub risks: Vec<RiskRow>,
    /// Things the plan covers that are neither a requirement nor a risk.
    pub other: Vec<String>,
}

impl PlanReport<'_> {
    /// True when something it covers is not yet tested, or a test fails.
    pub fn has_gaps(&self) -> bool {
        self.requirements
            .iter()
            .any(|r| !r.unverified.is_empty() || !r.failing.is_empty())
            || self.risks.iter().any(|r| r.live && !r.tested)
    }
}

fn scenario_ids(node: &Node) -> Vec<String> {
    node.requirements
        .iter()
        .flat_map(|r| &r.scenarios)
        .filter_map(|s| s.anchor.as_deref())
        .filter(|a| id::is_valid_segment(a))
        .map(|a| format!("{}.{a}", node.id))
        .collect()
}

/// The plans, read against which scenarios are unverified (`unverified`, as
/// `Coverage` lists them) and which fail (`failing`).
// spec: REQ-TESTPLAN-0002
pub fn report<'a>(
    graph: &'a Graph,
    unverified: &BTreeSet<String>,
    failing: &BTreeSet<String>,
) -> Vec<PlanReport<'a>> {
    let verified_whole = |req: &Node| -> bool {
        let ids = scenario_ids(req);
        !ids.is_empty() && ids.iter().all(|s| !unverified.contains(s))
    };
    graph
        .nodes
        .iter()
        .filter(|n| n.ty == "test-plan")
        .map(|plan| {
            let mut out = PlanReport {
                node: plan,
                requirements: Vec::new(),
                risks: Vec::new(),
                other: Vec::new(),
            };
            for target in plan.links.get("covers").into_iter().flatten() {
                match graph.get(target) {
                    Some(n) if n.ty == "requirement" => {
                        let ids = scenario_ids(n);
                        out.requirements.push(RequirementRow {
                            id: n.id.clone(),
                            scenarios: ids.len(),
                            unverified: ids
                                .iter()
                                .filter(|s| unverified.contains(*s))
                                .cloned()
                                .collect(),
                            failing: ids
                                .iter()
                                .filter(|s| failing.contains(*s))
                                .cloned()
                                .collect(),
                        });
                    }
                    Some(n) if n.ty == "risk" => {
                        let mitigations: Vec<String> = n
                            .links
                            .get("mitigated-by")
                            .into_iter()
                            .flatten()
                            .map(|t| id::base_of(t).to_string())
                            .collect();
                        let tested = !mitigations.is_empty()
                            && mitigations.iter().all(|m| {
                                graph
                                    .get(m)
                                    .is_some_and(|r| r.ty == "requirement" && verified_whole(r))
                            });
                        out.risks.push(RiskRow {
                            id: n.id.clone(),
                            level: risks::level(n.likelihood.as_deref(), n.impact.as_deref()),
                            live: risks::is_live(n.status.as_deref()),
                            mitigations,
                            tested,
                        });
                    }
                    _ => out.other.push(target.clone()),
                }
            }
            out
        })
        .collect()
}

/// The nodes that an approved plan covers.
fn covered_by_approved(graph: &Graph) -> BTreeSet<String> {
    graph
        .nodes
        .iter()
        .filter(|n| n.ty == "test-plan" && n.status.as_deref() == Some("approved"))
        .flat_map(|n| n.links.get("covers").into_iter().flatten())
        .map(|t| id::base_of(t).to_string())
        .collect()
}

// spec: REQ-TESTPLAN-0001
pub fn check(graph: &Graph) -> Vec<Finding> {
    let mut out = Vec::new();
    let approved = graph
        .nodes
        .iter()
        .any(|n| n.ty == "test-plan" && n.status.as_deref() == Some("approved"));
    for node in graph.nodes.iter().filter(|n| n.ty == "test-plan") {
        let at = |f: Finding| f.at(&node.path, Some(1)).node(&node.id);
        match node.status.as_deref() {
            Some(s) if STATUSES.contains(&s) => {}
            Some(s) => out.push(at(Finding::error(
                "E040",
                format!(
                    "test plan status {s:?} is not one of {}",
                    STATUSES.join(", ")
                ),
            ))),
            None => out.push(at(Finding::error("E040", "test plan has no status"))),
        }
        if let Some(d) = &node.date
            && day_number(d).is_none()
        {
            out.push(at(Finding::error(
                "E040",
                format!("test plan `date` {d:?} is not a date as YYYY-MM-DD"),
            )));
        }
        if node.links.get("covers").is_none_or(Vec::is_empty) {
            out.push(at(Finding::warning(
                "W020",
                "test plan covers nothing; link requirements or risks with `covers`",
            )));
        }
    }
    if approved {
        let covered = covered_by_approved(graph);
        for n in &graph.nodes {
            let gap = match n.ty.as_str() {
                "requirement" if n.status.as_deref() == Some("accepted") => Some((
                    "W021",
                    "requirement is accepted but no approved test plan covers it",
                )),
                "risk"
                    if risks::is_live(n.status.as_deref())
                        && matches!(
                            risks::level(n.likelihood.as_deref(), n.impact.as_deref()),
                            Some("high" | "critical")
                        ) =>
                {
                    Some((
                        "W022",
                        "risk is live at a high level but no approved test plan covers it",
                    ))
                }
                _ => None,
            };
            if let Some((code, text)) = gap
                && !covered.contains(&n.id)
            {
                out.push(
                    Finding::warning(code, text)
                        .at(&n.path, Some(1))
                        .node(&n.id),
                );
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse::parse_file_text;
    use std::path::Path;

    fn graph(files: &[(&str, &str)]) -> Graph {
        let nodes = files
            .iter()
            .map(|(name, text)| parse_file_text(Path::new(name), text).0.expect("a node"))
            .collect();
        Graph::from_nodes(nodes)
    }

    const REQ: &str = "---\nid: REQ-A-0001\ntype: requirement\nstatus: accepted\n---\n# REQ-A-0001: A\n\n### Requirement: A thing\nThe system SHALL do it.\n\n#### Scenario: One {#S1}\n- **WHEN** a\n- **THEN** b\n\n#### Scenario: Two {#S2}\n- **WHEN** c\n- **THEN** d\n";

    fn plan(status: &str, covers: &str) -> String {
        format!(
            "---\nid: TP-0001\ntype: test-plan\nstatus: {status}\nlinks:\n  covers: [{covers}]\n---\n# TP-0001: Plan\n"
        )
    }

    fn codes(g: &Graph) -> Vec<&'static str> {
        check(g).iter().map(|f| f.code).collect()
    }

    // verifies: REQ-TESTPLAN-0001.S1
    #[test]
    fn a_plan_needs_a_valid_status_and_something_to_cover() {
        let bad = plan("someday", "");
        assert_eq!(codes(&graph(&[("p.md", bad.as_str())])), ["E040", "W020"]);
        let ok = plan("draft", "REQ-A-0001");
        assert!(codes(&graph(&[("p.md", ok.as_str()), ("r.md", REQ)])).is_empty());
    }

    // verifies: REQ-TESTPLAN-0001.S2
    #[test]
    fn gaps_are_reported_only_when_the_project_has_an_approved_plan() {
        let draft = plan("draft", "");
        let g = graph(&[("p.md", draft.as_str()), ("r.md", REQ)]);
        assert!(!codes(&g).contains(&"W021"));
        let approved = plan("approved", "");
        let g = graph(&[("p.md", approved.as_str()), ("r.md", REQ)]);
        assert!(codes(&g).contains(&"W021"));
        let covering = plan("approved", "REQ-A-0001");
        let g = graph(&[("p.md", covering.as_str()), ("r.md", REQ)]);
        assert!(!codes(&g).contains(&"W021"));
        let risk = "---\nid: RISK-0001\ntype: risk\nstatus: open\nlikelihood: high\nimpact: high\nowner: a\n---\n# RISK-0001: R\n";
        let g = graph(&[("p.md", covering.as_str()), ("r.md", REQ), ("k.md", risk)]);
        assert!(codes(&g).contains(&"W022"));
    }

    // verifies: REQ-TESTPLAN-0002.S1
    #[test]
    fn the_report_reads_a_plan_against_the_tests_and_the_risks() {
        let risk = "---\nid: RISK-0001\ntype: risk\nstatus: open\nlikelihood: high\nimpact: high\nowner: a\nlinks:\n  mitigated-by: [REQ-A-0001]\n---\n# RISK-0001: R\n";
        let p = plan("approved", "REQ-A-0001, RISK-0001, ADR-0001");
        let g = graph(&[("p.md", p.as_str()), ("r.md", REQ), ("k.md", risk)]);
        let unverified: BTreeSet<String> = ["REQ-A-0001.S2".to_string()].into();
        let failing: BTreeSet<String> = ["REQ-A-0001.S1".to_string()].into();
        let reports = report(&g, &unverified, &failing);
        assert_eq!(reports.len(), 1);
        let r = &reports[0];
        assert_eq!(r.requirements[0].scenarios, 2);
        let row = &r.requirements[0];
        assert_eq!(row.unverified.len(), 1);
        assert!(row.unverified.contains(&"REQ-A-0001.S2".to_string()));
        assert_eq!(row.failing.len(), 1);
        assert!(row.failing.contains(&"REQ-A-0001.S1".to_string()));
        assert!(
            !r.risks[0].tested,
            "a scenario of its mitigation is not verified"
        );
        assert_eq!(r.other, ["ADR-0001"]);
        assert!(r.has_gaps());
        let clean = report(&g, &BTreeSet::new(), &BTreeSet::new());
        assert!(clean[0].risks[0].tested);
        assert!(!clean[0].has_gaps());
    }
}
