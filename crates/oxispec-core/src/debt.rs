//! Tech debt: a backlog of known shortcuts, kept as nodes of type `debt`.
//!
//! An item has a status, a severity, an owner, an optional due date, a cost
//! (what paying it off takes) and an interest (what it costs while it stays),
//! and an `arises-from` link to the requirement, decision or component it
//! comes from. `date` is when it was opened or last reviewed.
//!
//! E032 the status, severity or a date is missing or not valid
//! W013 an item that is still to be paid has no owner
//! W014 an item is past its due date
//! W015 an item has not been reviewed for longer than `debt.stale_days`
//! W016 an item has no `arises-from` link
//!
//! Items that are `resolved` or `accepted` (the risk is taken on purpose) are
//! not live: they raise none of W013 to W016 and do not count against limits.

use std::collections::BTreeMap;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::config::DebtConfig;
use crate::finding::Finding;
use crate::graph::Graph;
use crate::node::Node;

pub const STATUSES: &[&str] = &["open", "planned", "in-progress", "resolved", "accepted"];

/// From the least to the most serious.
pub const SEVERITIES: &[&str] = &["low", "medium", "high", "critical"];

/// True when the item still has to be paid off.
pub fn is_live(status: Option<&str>) -> bool {
    !matches!(status, Some("resolved" | "accepted"))
}

/// 0 for low (and for anything unknown) up to 3 for critical.
pub fn rank(severity: Option<&str>) -> usize {
    SEVERITIES
        .iter()
        .position(|s| Some(*s) == severity)
        .unwrap_or(0)
}

/// Today as `YYYY-MM-DD`, in UTC.
pub fn today() -> String {
    crate::change::utc_date(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| d.as_secs()),
    )
}

/// Days since 1970-01-01 for a `YYYY-MM-DD` date, or `None` if it is not one.
pub fn day_number(date: &str) -> Option<i64> {
    let bytes = date.as_bytes();
    if bytes.len() != 10 || bytes[4] != b'-' || bytes[7] != b'-' {
        return None;
    }
    let year: i64 = date[..4].parse().ok()?;
    let month: i64 = date[5..7].parse().ok()?;
    let day: i64 = date[8..].parse().ok()?;
    if !(1..=12).contains(&month) || !(1..=31).contains(&day) {
        return None;
    }
    // Days from civil, from Howard Hinnant's date algorithms.
    let y = year - i64::from(month <= 2);
    let era = y.div_euclid(400);
    let year_of_era = y.rem_euclid(400);
    let shifted = if month > 2 { month - 3 } else { month + 9 };
    let day_of_year = (153 * shifted + 2) / 5 + day - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    Some(era * 146_097 + day_of_era - 719_468)
}

/// One item of the backlog, with what is worked out from the dates.
#[derive(Debug, Clone)]
pub struct Item<'a> {
    pub node: &'a Node,
    pub live: bool,
    /// Days since `date`, when it has a valid one.
    pub age_days: Option<i64>,
    pub overdue: bool,
    pub stale: bool,
}

fn item<'a>(node: &'a Node, cfg: &DebtConfig, today: i64) -> Item<'a> {
    let live = is_live(node.status.as_deref());
    let age_days = node
        .date
        .as_deref()
        .and_then(day_number)
        .map(|opened| today - opened);
    let overdue = live
        && node
            .due
            .as_deref()
            .and_then(day_number)
            .is_some_and(|due| due < today);
    let stale = live && age_days.is_some_and(|a| a > i64::from(cfg.stale_days));
    Item {
        node,
        live,
        age_days,
        overdue,
        stale,
    }
}

/// The debt items, the most serious first and, among those, the oldest first.
/// With `all`, items that are resolved or accepted are included, last.
pub fn backlog<'a>(graph: &'a Graph, today: &str, all: bool) -> Vec<Item<'a>> {
    let today = day_number(today).unwrap_or(0);
    let mut items: Vec<Item> = graph
        .nodes
        .iter()
        .filter(|n| n.ty == "debt")
        .map(|n| item(n, &graph.debt, today))
        .filter(|i| all || i.live)
        .collect();
    items.sort_by(|a, b| {
        b.live
            .cmp(&a.live)
            .then(rank(b.node.severity.as_deref()).cmp(&rank(a.node.severity.as_deref())))
            .then(b.age_days.cmp(&a.age_days))
            .then(a.node.id.cmp(&b.node.id))
    });
    items
}

/// How many items are still to be paid, by severity. Severities with none are
/// left out.
pub fn live_counts(graph: &Graph) -> BTreeMap<String, usize> {
    let mut counts = BTreeMap::new();
    for node in graph.nodes.iter().filter(|n| n.ty == "debt") {
        if is_live(node.status.as_deref()) {
            let severity = node.severity.as_deref().unwrap_or("low");
            *counts.entry(severity.to_string()).or_default() += 1;
        }
    }
    counts
}

/// A severity whose limit is exceeded: `(severity, live items, limit)`.
pub fn over_limits(graph: &Graph) -> Vec<(String, usize, u32)> {
    let counts = live_counts(graph);
    let mut over = Vec::new();
    for severity in SEVERITIES.iter().rev() {
        if let Some(limit) = graph.debt.limits.get(*severity) {
            let live = counts.get(*severity).copied().unwrap_or(0);
            if live > *limit as usize {
                over.push((severity.to_string(), live, *limit));
            }
        }
    }
    over
}

/// Checks the debt items of the graph against today's date.
pub fn check(graph: &Graph) -> Vec<Finding> {
    check_at(graph, &today())
}

pub fn check_at(graph: &Graph, today: &str) -> Vec<Finding> {
    let mut out = Vec::new();
    let now = day_number(today).unwrap_or(0);
    for node in graph.nodes.iter().filter(|n| n.ty == "debt") {
        let at = |f: Finding| f.at(&node.path, Some(1)).node(&node.id);
        let bad = |msg: String| at(Finding::error("E032", msg));
        match node.status.as_deref() {
            Some(s) if STATUSES.contains(&s) => {}
            Some(s) => out.push(bad(format!(
                "debt status {s:?} is not one of {}",
                STATUSES.join(", ")
            ))),
            None => out.push(bad("debt has no status".into())),
        }
        match node.severity.as_deref() {
            Some(s) if SEVERITIES.contains(&s) => {}
            Some(s) => out.push(bad(format!(
                "debt severity {s:?} is not one of {}",
                SEVERITIES.join(", ")
            ))),
            None => out.push(bad("debt has no severity".into())),
        }
        for (name, value) in [("date", &node.date), ("due", &node.due)] {
            if let Some(v) = value
                && day_number(v).is_none()
            {
                out.push(bad(format!(
                    "debt `{name}` {v:?} is not a date as YYYY-MM-DD"
                )));
            }
        }
        let item = item(node, &graph.debt, now);
        if !item.live {
            continue;
        }
        if node.owner.as_deref().is_none_or(|o| o.trim().is_empty()) {
            out.push(at(Finding::warning(
                "W013",
                "debt has no owner; someone has to answer for paying it off",
            )));
        }
        if item.overdue {
            out.push(at(Finding::warning(
                "W014",
                format!(
                    "debt was due {}; pay it off, move the date or accept the risk",
                    node.due.as_deref().unwrap_or("")
                ),
            )));
        }
        if let Some(age) = item.age_days.filter(|_| item.stale) {
            out.push(at(Finding::warning(
                "W015",
                format!(
                    "debt has not been reviewed for {age} days (the limit is {}); update `date` when you review it",
                    graph.debt.stale_days
                ),
            )));
        }
        if node.links.get("arises-from").is_none_or(Vec::is_empty) {
            out.push(at(Finding::warning(
                "W016",
                "debt does not say what it arises from; link it with `arises-from`",
            )));
        }
    }
    out
}

#[cfg(test)]
mod tests_of_debt {
    use super::*;
    use crate::parse::parse_file_text;
    use std::path::Path;

    fn graph(items: &[(&str, &str)], cfg: DebtConfig) -> Graph {
        let nodes = items
            .iter()
            .map(|(id, front)| {
                let text = format!("---\nid: {id}\ntype: debt\n{front}---\n# {id}: Item\n");
                parse_file_text(Path::new(&format!("specs/debt/{id}.md")), &text)
                    .0
                    .expect("a node")
            })
            .collect();
        Graph::from_nodes(nodes).with_debt(cfg)
    }

    const FULL: &str = "status: open\nseverity: high\nowner: ann\ndate: 2026-10-01\nlinks:\n  arises-from: REQ-A-0001\n";

    fn codes(graph: &Graph, today: &str) -> Vec<String> {
        check_at(graph, today)
            .iter()
            .map(|f| f.code.to_string())
            .collect()
    }

    #[test]
    fn dates_are_counted_in_days() {
        assert_eq!(day_number("1970-01-01"), Some(0));
        assert_eq!(day_number("2000-03-01"), Some(11_017));
        assert_eq!(
            day_number("2026-11-04").unwrap() - day_number("2026-10-04").unwrap(),
            31
        );
        for bad in ["2026-13-01", "2026-1-01", "tomorrow", "2026-00-10", ""] {
            assert_eq!(day_number(bad), None, "{bad}");
        }
        assert_eq!(
            day_number(&crate::change::utc_date(86_400 * 365)),
            Some(365)
        );
    }

    #[test]
    fn a_complete_item_raises_nothing() {
        let g = graph(&[("DEBT-0001", FULL)], DebtConfig::default());
        assert!(codes(&g, "2026-11-04").is_empty());
    }

    #[test]
    fn a_live_item_without_an_owner_or_a_source_is_warned_about() {
        let g = graph(
            &[("DEBT-0001", "status: open\nseverity: low\n")],
            DebtConfig::default(),
        );
        assert_eq!(codes(&g, "2026-11-04"), ["W013", "W016"]);
    }

    #[test]
    fn an_invalid_status_severity_or_date_is_an_error() {
        let g = graph(
            &[(
                "DEBT-0001",
                "status: later\nseverity: huge\nowner: a\ndue: soon\ndate: 2026-02-30x\nlinks:\n  arises-from: X\n",
            )],
            DebtConfig::default(),
        );
        let found = codes(&g, "2026-11-04");
        assert_eq!(
            found.iter().filter(|c| *c == "E032").count(),
            4,
            "{found:?}"
        );
        let g = graph(&[("DEBT-0001", "owner: a\n")], DebtConfig::default());
        assert_eq!(
            codes(&g, "2026-11-04")
                .iter()
                .filter(|c| *c == "E032")
                .count(),
            2,
            "no status and no severity"
        );
    }

    #[test]
    fn overdue_and_stale_apply_only_to_live_items() {
        let late = format!("{FULL}due: 2026-10-15\n");
        let g = graph(&[("DEBT-0001", late.as_str())], DebtConfig::default());
        assert_eq!(codes(&g, "2026-11-04"), ["W014"]);
        assert!(codes(&g, "2026-10-15").is_empty(), "due today is not late");
        // 2026-10-01 is 34 days before 2026-11-04.
        let cfg = DebtConfig {
            stale_days: 30,
            ..DebtConfig::default()
        };
        let g = graph(&[("DEBT-0001", FULL)], cfg.clone());
        assert_eq!(codes(&g, "2026-11-04"), ["W015"]);
        for done in ["resolved", "accepted"] {
            let closed = FULL.replace("status: open", &format!("status: {done}"));
            let g = graph(&[("DEBT-0001", closed.as_str())], cfg.clone());
            assert!(codes(&g, "2026-11-04").is_empty(), "{done}");
        }
    }

    #[test]
    fn the_backlog_puts_the_most_serious_and_the_oldest_first() {
        let g = graph(
            &[
                (
                    "DEBT-0001",
                    "status: open\nseverity: low\ndate: 2025-01-01\n",
                ),
                (
                    "DEBT-0002",
                    "status: open\nseverity: critical\ndate: 2026-10-01\n",
                ),
                (
                    "DEBT-0003",
                    "status: planned\nseverity: high\ndate: 2026-01-01\n",
                ),
                (
                    "DEBT-0004",
                    "status: planned\nseverity: high\ndate: 2025-06-01\n",
                ),
                ("DEBT-0005", "status: resolved\nseverity: critical\n"),
            ],
            DebtConfig::default(),
        );
        let ids = |all| {
            backlog(&g, "2026-11-04", all)
                .iter()
                .map(|i| i.node.id.clone())
                .collect::<Vec<_>>()
        };
        assert_eq!(
            ids(false),
            ["DEBT-0002", "DEBT-0004", "DEBT-0003", "DEBT-0001"]
        );
        assert_eq!(
            ids(true),
            [
                "DEBT-0002",
                "DEBT-0004",
                "DEBT-0003",
                "DEBT-0001",
                "DEBT-0005"
            ]
        );
    }

    #[test]
    fn limits_count_live_items_by_severity() {
        let mut cfg = DebtConfig::default();
        cfg.limits.insert("critical".into(), 0);
        cfg.limits.insert("high".into(), 1);
        let g = graph(
            &[
                ("DEBT-0001", "status: open\nseverity: critical\n"),
                ("DEBT-0002", "status: open\nseverity: high\n"),
                ("DEBT-0003", "status: planned\nseverity: high\n"),
                ("DEBT-0004", "status: resolved\nseverity: high\n"),
                ("DEBT-0005", "status: open\nseverity: medium\n"),
            ],
            cfg,
        );
        assert_eq!(
            over_limits(&g),
            [("critical".to_string(), 1, 0), ("high".to_string(), 2, 1)]
        );
        assert_eq!(live_counts(&g).get("medium"), Some(&1));
    }
}
