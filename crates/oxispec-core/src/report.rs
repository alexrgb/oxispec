//! Reports for CI: what a change introduced, as a pull request comment
//! (markdown), SARIF for code scanning, and JUnit for test dashboards.
//! This module only formats. The command line gathers the inputs.

use std::fmt::Write;
use std::path::PathBuf;

use serde_json::{Value, json};

use crate::diff::GraphDiff;
use crate::finding::{CODES, Finding, Severity};
use crate::release::Bump;

/// A node that a change touches, directly or through a link.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Touched {
    pub id: String,
    pub ty: String,
    pub path: PathBuf,
}

/// Marks the comment, so a workflow can find it again and update it.
pub const COMMENT_MARKER: &str = "<!-- oxispec-ci -->";

pub struct Report<'a> {
    /// What the change is compared with, for people.
    pub base: String,
    /// Nodes in the specs being checked.
    pub total_nodes: usize,
    /// The changed nodes and the nodes that link to them.
    pub touched: Vec<Touched>,
    pub diff: &'a GraphDiff,
    /// Errors and warnings that were already there and are not shown.
    pub existing: usize,
    /// How big the change is, for the version a release would need.
    pub bump: Bump,
    /// Warnings fail the run too.
    pub strict: bool,
    /// The entities that changed, and what depends on each.
    pub entities: Vec<crate::entities::Impact>,
}

impl Report<'_> {
    fn count(&self, severity: Severity) -> usize {
        self.diff
            .new_findings
            .iter()
            .filter(|f| f.severity == severity)
            .count()
    }

    pub fn new_errors(&self) -> usize {
        self.count(Severity::Error)
    }

    pub fn new_warnings(&self) -> usize {
        self.count(Severity::Warning)
    }

    /// The run fails on new errors, and on new warnings in strict mode.
    pub fn failed(&self) -> bool {
        self.new_errors() > 0 || (self.strict && self.new_warnings() > 0)
    }

    fn location(finding: &Finding) -> Option<String> {
        let path = finding
            .path
            .as_ref()?
            .display()
            .to_string()
            .replace('\\', "/");
        Some(match finding.line {
            Some(line) => format!("{path}:{line}"),
            None => path,
        })
    }
}

fn plural(n: usize, one: &str, many: &str) -> String {
    format!("{n} {}", if n == 1 { one } else { many })
}

/// A table cell: no pipes or line breaks.
fn cell(text: &str) -> String {
    text.replace('|', "\\|").replace(['\n', '\r'], " ")
}

/// The pull request comment.
pub fn markdown(report: &Report) -> String {
    let diff = report.diff;
    let mut out = format!("{COMMENT_MARKER}\n## OxiSpec\n\n");
    let changed = diff.added.len() + diff.removed.len() + diff.changed.len();
    if changed == 0 {
        out.push_str("No spec nodes changed.\n");
    } else {
        let _ = write!(
            out,
            "**{}** changed ({} added, {} removed, {} edited)",
            plural(changed, "node", "nodes"),
            diff.added.len(),
            diff.removed.len(),
            diff.changed.len()
        );
        let linked = report.touched.len().saturating_sub(changed);
        if linked > 0 {
            let _ = write!(
                out,
                ", and {} through links",
                plural(linked, "more is reached", "more are reached")
            );
        }
        let _ = writeln!(out, ". Size of the change: **{:?}**.", report.bump);
    }
    out.push('\n');

    let (errors, warnings) = (report.new_errors(), report.new_warnings());
    if errors + warnings == 0 {
        out.push_str("✅ No new findings");
    } else {
        let _ = write!(
            out,
            "❌ New findings: **{}**, **{}**",
            plural(errors, "error", "errors"),
            plural(warnings, "warning", "warnings")
        );
    }
    if !diff.fixed_findings.is_empty() {
        let _ = write!(out, ". Fixed: {}", diff.fixed_findings.len());
    }
    if report.existing > 0 {
        let _ = write!(
            out,
            ". {} already there and not shown",
            plural(report.existing, "finding was", "findings were")
        );
    }
    out.push_str(".\n");

    if !diff.new_findings.is_empty() {
        out.push_str("\n| | Code | Where | What |\n|---|---|---|---|\n");
        for f in &diff.new_findings {
            let icon = if f.severity == Severity::Error {
                "🔴"
            } else {
                "🟡"
            };
            let _ = writeln!(
                out,
                "| {icon} | {} | {} | {} |",
                f.code,
                cell(&Report::location(f).unwrap_or_else(|| f.id.clone().unwrap_or_default())),
                cell(&f.message)
            );
        }
    }

    if !report.entities.is_empty() {
        let _ = writeln!(
            out,
            "\n**{} changed.** What depends on them may need a second look:\n",
            plural(report.entities.len(), "entity", "entities")
        );
        for e in &report.entities {
            let breaking = e.changes.iter().any(|c| c.breaking);
            let what: Vec<String> = e.changes.iter().map(|c| c.detail.clone()).collect();
            let _ = writeln!(
                out,
                "- {} `{}` ({}): {}",
                if breaking { "⚠️" } else { "ℹ️" },
                e.id,
                e.name,
                cell(&what.join("; "))
            );
            if e.affected.is_empty() {
                out.push_str("  - nothing depends on it yet\n");
            } else {
                let shown: Vec<String> = e
                    .affected
                    .iter()
                    .take(10)
                    .map(|a| format!("`{}`", a.id))
                    .collect();
                let more = e.affected.len().saturating_sub(10);
                let _ = write!(
                    out,
                    "  - {} depend on it: {}",
                    e.affected.len(),
                    shown.join(", ")
                );
                if more > 0 {
                    let _ = write!(out, " and {more} more");
                }
                out.push('\n');
            }
        }
    }

    if changed > 0 {
        out.push_str("\n<details><summary>Changed nodes</summary>\n\n");
        for n in &diff.added {
            let _ = writeln!(out, "- ➕ `{}` ({})", n.id, n.ty);
        }
        for n in &diff.removed {
            let _ = writeln!(out, "- ➖ `{}` ({})", n.id, n.ty);
        }
        for n in &diff.changed {
            let what: Vec<String> = n.changes.iter().map(|c| c.describe()).collect();
            let _ = writeln!(out, "- ✏️ `{}`: {}", n.id, cell(&what.join("; ")));
        }
        out.push_str("\n</details>\n");
    }
    out
}

/// SARIF 2.1.0, with the new findings only, so code scanning shows what the
/// change introduced.
pub fn sarif(report: &Report, tool_version: &str) -> Value {
    let used: Vec<&str> = {
        let mut codes: Vec<&str> = report.diff.new_findings.iter().map(|f| f.code).collect();
        codes.sort_unstable();
        codes.dedup();
        codes
    };
    let rules: Vec<Value> = used
        .iter()
        .map(|code| {
            let text = CODES
                .iter()
                .find(|(c, _)| c == code)
                .map_or("", |(_, text)| *text);
            json!({"id": code, "shortDescription": {"text": text}})
        })
        .collect();
    let results: Vec<Value> = report
        .diff
        .new_findings
        .iter()
        .map(|f| {
            let level = match f.severity {
                Severity::Error => "error",
                Severity::Warning => "warning",
                Severity::Info => "note",
            };
            let mut result = json!({
                "ruleId": f.code,
                "ruleIndex": used.iter().position(|c| *c == f.code),
                "level": level,
                "message": {"text": f.message},
            });
            if let Some(path) = &f.path {
                let mut physical = json!({
                    "artifactLocation": {"uri": path.display().to_string().replace('\\', "/")},
                });
                if let Some(line) = f.line {
                    physical["region"] = json!({"startLine": line});
                }
                result["locations"] = json!([{"physicalLocation": physical}]);
            }
            if let Some(id) = &f.id {
                result["partialFingerprints"] =
                    json!({"oxispecNode/v1": format!("{}|{id}|{}", f.code, f.message)});
            }
            result
        })
        .collect();
    json!({
        "$schema": "https://json.schemastore.org/sarif-2.1.0.json",
        "version": "2.1.0",
        "runs": [{
            "tool": {"driver": {
                "name": "oxispec",
                "version": tool_version,
                "informationUri": "https://github.com/alexrgb/oxispec",
                "rules": rules,
            }},
            "results": results,
        }],
    })
}

/// XML text: the five special characters, and nothing XML 1.0 forbids.
fn xml(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            '\t' | '\n' | '\r' => out.push(c),
            c if (c as u32) < 0x20 || matches!(c as u32, 0xFFFE | 0xFFFF) => {}
            c => out.push(c),
        }
    }
    out
}

/// JUnit XML: one test case per touched node, failed when a new finding is
/// about it, plus one per new finding that belongs to no touched node.
pub fn junit(report: &Report) -> String {
    struct Case {
        class: String,
        name: String,
        failures: Vec<String>,
    }
    let mut cases: Vec<Case> = report
        .touched
        .iter()
        .map(|t| Case {
            class: t.ty.clone(),
            name: t.id.clone(),
            failures: Vec::new(),
        })
        .collect();
    for f in &report.diff.new_findings {
        let fails = f.severity == Severity::Error || report.strict;
        let text = format!("{}: {}", f.code, f.message);
        let owner = cases.iter_mut().find(|c| {
            f.id.as_deref() == Some(c.name.as_str())
                || f.path.as_ref().is_some_and(|p| {
                    report
                        .touched
                        .iter()
                        .any(|t| t.id == c.name && &t.path == p)
                })
        });
        match owner {
            Some(case) if fails => case.failures.push(text),
            Some(_) => {}
            None => cases.push(Case {
                class: "finding".into(),
                name: format!(
                    "{} {}",
                    f.code,
                    Report::location(f)
                        .or_else(|| f.id.clone())
                        .unwrap_or_default()
                ),
                failures: if fails { vec![text] } else { Vec::new() },
            }),
        }
    }
    let failures = cases.iter().filter(|c| !c.failures.is_empty()).count();
    let mut out = String::from("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
    let _ = writeln!(
        out,
        "<testsuite name=\"oxispec ci\" tests=\"{}\" failures=\"{failures}\" errors=\"0\">",
        cases.len()
    );
    for case in &cases {
        let _ = write!(
            out,
            "  <testcase classname=\"{}\" name=\"{}\"",
            xml(&case.class),
            xml(&case.name)
        );
        if case.failures.is_empty() {
            out.push_str("/>\n");
        } else {
            out.push_str(">\n");
            for text in &case.failures {
                let _ = writeln!(out, "    <failure message=\"{}\"/>", xml(text));
            }
            out.push_str("  </testcase>\n");
        }
    }
    out.push_str("</testsuite>\n");
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diff::diff_graphs;
    use crate::graph::Graph;
    use crate::parse::parse_file_text;
    use std::path::Path;

    const REQ: &str = "### Requirement: X\nThe system SHALL work.\n\n#### Scenario: S {#S1}\n- **WHEN** a\n- **THEN** b\n";

    fn graph(files: &[(&str, String)]) -> Graph {
        Graph::from_nodes(
            files
                .iter()
                .filter_map(|(p, t)| parse_file_text(Path::new(p), t).0)
                .collect(),
        )
    }

    fn req(id: &str, extra: &str) -> String {
        format!("---\nid: {id}\ntype: requirement\n{extra}---\n{REQ}")
    }

    /// Before: one requirement. After: a second one with a dangling link, and
    /// a warning about an unknown link kind.
    fn diff() -> GraphDiff {
        let old = graph(&[("a.md", req("REQ-A-0001", ""))]);
        let new = graph(&[
            ("a.md", req("REQ-A-0001", "")),
            (
                "b.md",
                req(
                    "REQ-A-0002",
                    "links:\n  refines: REQ-NOPE-0001\n  wibble: REQ-A-0001\n",
                ),
            ),
        ]);
        let mut diff = diff_graphs(&old, &new);
        // A message with characters that need care in every format.
        diff.new_findings[0].message = "a <b> & \"c\" | d".to_string();
        diff
    }

    fn report(diff: &GraphDiff, strict: bool) -> Report<'_> {
        Report {
            base: "main".into(),
            total_nodes: 2,
            touched: vec![Touched {
                id: "REQ-A-0002".into(),
                ty: "requirement".into(),
                path: "b.md".into(),
            }],
            diff,
            existing: 3,
            bump: Bump::Minor,
            strict,
            entities: Vec::new(),
        }
    }

    #[test]
    fn a_run_fails_on_new_errors_and_on_warnings_only_when_strict() {
        let d = diff();
        assert_eq!(report(&d, false).new_errors(), 1);
        assert_eq!(report(&d, false).new_warnings(), 1);
        assert!(report(&d, false).failed());

        let warning_only = GraphDiff {
            added: vec![],
            removed: vec![],
            changed: vec![],
            new_findings: d
                .new_findings
                .iter()
                .filter(|f| f.severity == Severity::Warning)
                .cloned()
                .collect(),
            fixed_findings: vec![],
        };
        assert!(!report(&warning_only, false).failed());
        assert!(report(&warning_only, true).failed());
    }

    #[test]
    fn the_comment_is_marked_and_escapes_table_cells() {
        let d = diff();
        let text = markdown(&report(&d, false));
        assert!(text.starts_with(COMMENT_MARKER));
        assert!(
            text.contains("1 node** changed (1 added, 0 removed, 0 edited)"),
            "{text}"
        );
        assert!(
            text.contains("❌ New findings: **1 error**, **1 warning**"),
            "{text}"
        );
        assert!(
            text.contains("3 findings were already there and not shown"),
            "{text}"
        );
        assert!(text.contains("a <b> & \"c\" \\| d"), "{text}");
        assert!(text.contains("- ➕ `REQ-A-0002` (requirement)"));
    }

    #[test]
    fn a_clean_change_says_so() {
        let g = graph(&[("a.md", req("REQ-A-0001", ""))]);
        let d = diff_graphs(&g, &g);
        let mut r = report(&d, false);
        r.touched.clear();
        r.existing = 0;
        let text = markdown(&r);
        assert!(text.contains("No spec nodes changed."));
        assert!(text.contains("✅ No new findings."), "{text}");
        assert!(!text.contains("<details>"));
    }

    #[test]
    fn sarif_has_the_new_findings_with_locations_and_rules() {
        let d = diff();
        let v = sarif(&report(&d, false), "1.2.3");
        assert_eq!(v["version"], "2.1.0");
        let run = &v["runs"][0];
        assert_eq!(run["tool"]["driver"]["version"], "1.2.3");
        let results = run["results"].as_array().unwrap();
        assert_eq!(results.len(), 2);
        let error = results.iter().find(|r| r["level"] == "error").unwrap();
        assert_eq!(error["ruleId"], "E002");
        assert_eq!(
            error["locations"][0]["physicalLocation"]["artifactLocation"]["uri"],
            "b.md"
        );
        let rules = run["tool"]["driver"]["rules"].as_array().unwrap();
        let index = error["ruleIndex"].as_u64().unwrap() as usize;
        assert_eq!(rules[index]["id"], "E002");
        assert_eq!(
            rules[index]["shortDescription"]["text"],
            "A link points to a node that does not exist"
        );
    }

    #[test]
    fn junit_has_a_case_per_touched_node_and_escapes_text() {
        let d = diff();
        let text = junit(&report(&d, false));
        assert!(text.contains("tests=\"1\" failures=\"1\""), "{text}");
        assert!(text.contains("<testcase classname=\"requirement\" name=\"REQ-A-0002\">"));
        assert!(
            text.contains("a &lt;b&gt; &amp; &quot;c&quot; | d"),
            "{text}"
        );
        assert!(!text.contains("<b>"));

        // A warning does not fail a test unless the run is strict.
        let lenient = junit(&report(&d, false));
        assert_eq!(lenient.matches("<failure").count(), 1);
        let strict = junit(&report(&d, true));
        assert_eq!(strict.matches("<failure").count(), 2);
    }

    #[test]
    fn xml_drops_characters_it_cannot_hold() {
        assert_eq!(xml("a\u{1}b\u{0}c\u{FFFE}d"), "abcd");
        assert_eq!(xml("tab\there"), "tab\there");
    }
}
