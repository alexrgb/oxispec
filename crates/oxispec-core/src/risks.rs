//! The risk register: known risks to the application, kept as nodes of type
//! `risk`.
//!
//! A risk has a status, a likelihood, an impact, an owner, a date of the last
//! review and optionally a date by which it must be reviewed again (`review`).
//! Its level is worked out from likelihood and impact, never written by hand.
//! `mitigated-by` links name the requirement, decision or test that deals
//! with it, and `arises-from` names what it comes from. A risk the team takes
//! on purpose is `accepted`, and needs a person's name in `approved_by`.
//!
//! E038 the status, likelihood or impact is missing or not valid, or a date is not one
//! E039 an accepted risk has no `approved_by`
//! W017 a live risk has no owner
//! W018 a live risk is past its `review` date
//! W019 a risk that is mitigating or mitigated names nothing that mitigates it
//!
//! A risk is *live* while it is `open` or `mitigating`. A `mitigated`,
//! `accepted` or `closed` risk raises none of W017 to W018.

use crate::debt::day_number;
use crate::finding::Finding;
use crate::graph::Graph;
use crate::node::Node;

pub const STATUSES: &[&str] = &["open", "mitigating", "mitigated", "accepted", "closed"];
/// From the least to the most likely.
pub const LIKELIHOODS: &[&str] = &["low", "medium", "high"];
/// From the least to the most serious.
pub const IMPACTS: &[&str] = &["low", "medium", "high", "critical"];
/// From the least to the most serious.
pub const LEVELS: &[&str] = &["low", "medium", "high", "critical"];

/// True while the risk still needs work.
pub fn is_live(status: Option<&str>) -> bool {
    matches!(status, Some("open" | "mitigating"))
}

fn position(list: &[&str], value: Option<&str>) -> Option<usize> {
    list.iter().position(|s| Some(*s) == value)
}

/// The level that a likelihood and an impact give: the product of their
/// places on the scales (1 to 3 and 1 to 4). Up to 2 is low, up to 4 medium,
/// up to 8 high and above that critical. `None` when either is missing or
/// not valid.
// spec: REQ-RISK-0002
pub fn level(likelihood: Option<&str>, impact: Option<&str>) -> Option<&'static str> {
    let l = position(LIKELIHOODS, likelihood)? + 1;
    let i = position(IMPACTS, impact)? + 1;
    Some(match l * i {
        0..=2 => "low",
        3..=4 => "medium",
        5..=8 => "high",
        _ => "critical",
    })
}

/// 0 for low (and for anything unknown) up to 3 for critical.
pub fn rank(level: Option<&str>) -> usize {
    position(LEVELS, level).unwrap_or(0)
}

/// One risk of the register, with what is worked out.
#[derive(Debug, Clone)]
pub struct Item<'a> {
    pub node: &'a Node,
    pub live: bool,
    pub level: Option<&'static str>,
    /// Days since `date`, when it has a valid one.
    pub age_days: Option<i64>,
    pub review_overdue: bool,
}

fn item<'a>(node: &'a Node, today: i64) -> Item<'a> {
    let live = is_live(node.status.as_deref());
    Item {
        node,
        live,
        level: level(node.likelihood.as_deref(), node.impact.as_deref()),
        age_days: node.date.as_deref().and_then(day_number).map(|d| today - d),
        review_overdue: live
            && node
                .review
                .as_deref()
                .and_then(day_number)
                .is_some_and(|r| r < today),
    }
}

/// The risks, the most serious first and, among those, the oldest first.
/// With `all`, risks that are not live are included, last.
// spec: REQ-RISK-0004
pub fn register<'a>(graph: &'a Graph, today: &str, all: bool) -> Vec<Item<'a>> {
    let today = day_number(today).unwrap_or(0);
    let mut items: Vec<Item> = graph
        .nodes
        .iter()
        .filter(|n| n.ty == "risk")
        .map(|n| item(n, today))
        .filter(|i| all || i.live)
        .collect();
    items.sort_by(|a, b| {
        b.live
            .cmp(&a.live)
            .then(rank(b.level).cmp(&rank(a.level)))
            .then(b.age_days.cmp(&a.age_days))
            .then(a.node.id.cmp(&b.node.id))
    });
    items
}

/// Live risks at the critical level. A release is held back while there are any.
// spec: REQ-RISK-0005
pub fn critical(graph: &Graph) -> Vec<&Node> {
    graph
        .nodes
        .iter()
        .filter(|n| n.ty == "risk" && is_live(n.status.as_deref()))
        .filter(|n| level(n.likelihood.as_deref(), n.impact.as_deref()) == Some("critical"))
        .collect()
}

/// True when the project has any risk node.
pub fn has_risks(graph: &Graph) -> bool {
    graph.nodes.iter().any(|n| n.ty == "risk")
}

/// Checks the risks of the graph against today's date.
pub fn check(graph: &Graph) -> Vec<Finding> {
    check_at(graph, &crate::debt::today())
}

// spec: REQ-RISK-0001
// spec: REQ-RISK-0003
pub fn check_at(graph: &Graph, today: &str) -> Vec<Finding> {
    let mut out = Vec::new();
    let now = day_number(today).unwrap_or(0);
    for node in graph.nodes.iter().filter(|n| n.ty == "risk") {
        let at = |f: Finding| f.at(&node.path, Some(1)).node(&node.id);
        let bad = |msg: String| at(Finding::error("E038", msg));
        for (name, value, allowed) in [
            ("status", node.status.as_deref(), STATUSES),
            ("likelihood", node.likelihood.as_deref(), LIKELIHOODS),
            ("impact", node.impact.as_deref(), IMPACTS),
        ] {
            match value {
                Some(v) if allowed.contains(&v) => {}
                Some(v) => out.push(bad(format!(
                    "risk {name} {v:?} is not one of {}",
                    allowed.join(", ")
                ))),
                None => out.push(bad(format!("risk has no {name}"))),
            }
        }
        for (name, value) in [("date", &node.date), ("review", &node.review)] {
            if let Some(v) = value
                && day_number(v).is_none()
            {
                out.push(bad(format!(
                    "risk `{name}` {v:?} is not a date as YYYY-MM-DD"
                )));
            }
        }
        if node.status.as_deref() == Some("accepted")
            && node
                .approved_by
                .as_deref()
                .is_none_or(|a| a.trim().is_empty())
        {
            out.push(at(Finding::error(
                "E039",
                "an accepted risk needs `approved_by`: a person has to take it on",
            )));
        }
        let item = item(node, now);
        if item.live && node.owner.as_deref().is_none_or(|o| o.trim().is_empty()) {
            out.push(at(Finding::warning(
                "W017",
                "risk has no owner; someone has to answer for it",
            )));
        }
        if item.review_overdue {
            out.push(at(Finding::warning(
                "W018",
                format!(
                    "risk was to be reviewed by {}; review it and move `review` on",
                    node.review.as_deref().unwrap_or("")
                ),
            )));
        }
        if matches!(node.status.as_deref(), Some("mitigating" | "mitigated"))
            && node.links.get("mitigated-by").is_none_or(Vec::is_empty)
        {
            out.push(at(Finding::warning(
                "W019",
                "risk is being mitigated but says by what; link it with `mitigated-by`",
            )));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse::parse_file_text;
    use std::path::Path;

    fn graph(items: &[(&str, &str)]) -> Graph {
        let nodes = items
            .iter()
            .map(|(id, front)| {
                let text = format!("---\nid: {id}\ntype: risk\n{front}---\n# {id}: Risk\n");
                parse_file_text(Path::new(&format!("specs/risks/{id}.md")), &text)
                    .0
                    .expect("a node")
            })
            .collect();
        Graph::from_nodes(nodes)
    }

    fn codes(g: &Graph, today: &str) -> Vec<&'static str> {
        check_at(g, today).iter().map(|f| f.code).collect()
    }

    const FULL: &str = "status: open\nlikelihood: medium\nimpact: high\nowner: sec\ndate: 2026-11-01\nreview: 2026-12-01\n";

    // verifies: REQ-RISK-0001.S1
    #[test]
    fn invalid_fields_are_errors_and_a_complete_risk_raises_nothing() {
        assert!(codes(&graph(&[("RISK-0001", FULL)]), "2026-11-06").is_empty());
        let g = graph(&[(
            "RISK-0001",
            "status: later\nlikelihood: certain\nimpact: huge\nowner: a\ndate: x\nreview: soon\n",
        )]);
        let found = codes(&g, "2026-11-06");
        assert_eq!(
            found.iter().filter(|c| **c == "E038").count(),
            5,
            "{found:?}"
        );
        let g = graph(&[("RISK-0001", "owner: a\n")]);
        assert_eq!(
            codes(&g, "2026-11-06")
                .iter()
                .filter(|c| **c == "E038")
                .count(),
            3
        );
    }

    // verifies: REQ-RISK-0002.S1
    #[test]
    fn the_level_comes_from_likelihood_and_impact() {
        assert_eq!(level(Some("low"), Some("low")), Some("low"));
        assert_eq!(level(Some("low"), Some("critical")), Some("medium"));
        assert_eq!(level(Some("medium"), Some("high")), Some("high"));
        assert_eq!(level(Some("medium"), Some("critical")), Some("high"));
        assert_eq!(level(Some("high"), Some("high")), Some("critical"));
        assert_eq!(level(Some("high"), None), None);
        assert_eq!(level(Some("often"), Some("low")), None);
    }

    // verifies: REQ-RISK-0003.S1
    #[test]
    fn an_accepted_risk_needs_a_person_to_have_approved_it() {
        let accepted = FULL.replace("status: open", "status: accepted");
        let g = graph(&[("RISK-0001", accepted.as_str())]);
        assert_eq!(codes(&g, "2026-11-06"), ["E039"]);
        let signed = format!("{accepted}approved_by: Dana\n");
        let g = graph(&[("RISK-0001", signed.as_str())]);
        assert!(codes(&g, "2026-11-06").is_empty());
    }

    // verifies: REQ-RISK-0003.S2
    #[test]
    fn a_live_risk_is_held_to_account_and_a_closed_one_is_not() {
        let g = graph(&[("RISK-0001", "status: open\nlikelihood: low\nimpact: low\n")]);
        assert_eq!(codes(&g, "2026-11-06"), ["W017"]);
        let g = graph(&[("RISK-0001", FULL)]);
        assert_eq!(codes(&g, "2026-12-02"), ["W018"]);
        assert!(
            codes(&g, "2026-12-01").is_empty(),
            "the day itself is not late"
        );
        let closed = FULL.replace("status: open", "status: closed");
        let g = graph(&[("RISK-0001", closed.as_str())]);
        assert!(codes(&g, "2027-01-01").is_empty());
        let mitigating = FULL.replace("status: open", "status: mitigating");
        let g = graph(&[("RISK-0001", mitigating.as_str())]);
        assert_eq!(codes(&g, "2026-11-06"), ["W019"]);
        let linked = format!("{mitigating}links:\n  mitigated-by: [REQ-A-0001]\n");
        let g = graph(&[("RISK-0001", linked.as_str())]);
        assert!(codes(&g, "2026-11-06").is_empty());
    }

    // verifies: REQ-RISK-0004.S1
    #[test]
    fn the_register_puts_the_most_serious_and_the_oldest_first() {
        let g = graph(&[
            (
                "RISK-0001",
                "status: open\nlikelihood: low\nimpact: low\ndate: 2026-01-01\n",
            ),
            (
                "RISK-0002",
                "status: open\nlikelihood: high\nimpact: high\ndate: 2026-10-01\n",
            ),
            (
                "RISK-0003",
                "status: open\nlikelihood: high\nimpact: high\ndate: 2026-05-01\n",
            ),
            (
                "RISK-0004",
                "status: closed\nlikelihood: high\nimpact: critical\n",
            ),
        ]);
        let ids: Vec<_> = register(&g, "2026-11-06", false)
            .iter()
            .map(|i| i.node.id.clone())
            .collect();
        assert_eq!(ids, ["RISK-0003", "RISK-0002", "RISK-0001"]);
        let all = register(&g, "2026-11-06", true);
        assert_eq!(all.last().map(|i| i.node.id.as_str()), Some("RISK-0004"));
    }

    // verifies: REQ-RISK-0005.S1
    #[test]
    fn only_a_live_critical_risk_is_critical_for_a_release() {
        let g = graph(&[
            (
                "RISK-0001",
                "status: open\nlikelihood: high\nimpact: high\n",
            ),
            (
                "RISK-0002",
                "status: mitigated\nlikelihood: high\nimpact: critical\n",
            ),
            (
                "RISK-0003",
                "status: open\nlikelihood: low\nimpact: critical\n",
            ),
        ]);
        let ids: Vec<_> = critical(&g).iter().map(|n| n.id.clone()).collect();
        assert_eq!(ids, ["RISK-0001"]);
    }
}
