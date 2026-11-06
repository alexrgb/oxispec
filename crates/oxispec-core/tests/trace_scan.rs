//! Tests of the trace scan that carry real `verifies:` comments. Their
//! fixtures are built with `concat!` so that the scan does not read them as
//! annotations. The older tests of the scan, in `src/trace.rs`, hold sample
//! annotations as text and are left out of the scan.

use std::path::{Path, PathBuf};

use oxispec_core::graph::Graph;
use oxispec_core::trace::{Kind, coverage, scan_text};

fn scan(text: &str) -> Vec<(String, Kind, usize)> {
    scan_text(Path::new("x.rs"), text)
        .into_iter()
        .map(|a| (a.target, a.kind, a.line))
        .collect()
}

const REQ: &str = "---\nid: REQ-A-0001\ntype: requirement\nstatus: accepted\n---\n### Requirement: X\nThe system SHALL x.\n\n#### Scenario: One {#S1}\n- **WHEN** a\n- **THEN** b\n\n#### Scenario: Two {#S2}\n- **WHEN** c\n- **THEN** d\n";

fn graph() -> Graph {
    Graph::from_sources([(PathBuf::from("specs/a.md"), REQ.as_bytes().to_vec())])
}

// verifies: REQ-TRACE-0001.S1
#[test]
fn annotations_are_read_from_the_comments_of_many_languages() {
    let spec = concat!("// spe", "c: REQ-A-0001");
    let doc = concat!("/// spe", "c: REQ-A-0002, ADR-0007");
    let hash = concat!("# veri", "fies: REQ-A-0001.S1");
    let sql = concat!("-- spe", "c: REQ-A-0003");
    let found = scan(&format!("{spec}\n{doc}\n{hash}\n{sql}\n"));
    let targets: Vec<&str> = found.iter().map(|f| f.0.as_str()).collect();
    assert_eq!(
        targets,
        [
            "REQ-A-0001",
            "REQ-A-0002",
            "ADR-0007",
            "REQ-A-0001.S1",
            "REQ-A-0003"
        ]
    );
    assert_eq!(found[3].1, Kind::Verifies);
    assert_eq!(found[0].1, Kind::Implements);
}

// verifies: REQ-TRACE-0001.S2
#[test]
fn prose_and_lookalikes_are_not_annotations() {
    for text in [
        concat!("let spe", "c: Spec = load();"),
        concat!("// the spe", "c: see-above for details"),
        concat!("// spe", "c: do-it"),
        concat!("// myspe", "c: REQ-A-0001"),
        concat!("// spe", "c:"),
        concat!("// see https://example.com/a:", "REQ-A-0001"),
    ] {
        assert!(scan(text).is_empty(), "{text}");
    }
}

// verifies: REQ-TRACE-0003.S1
#[test]
fn a_marker_inside_a_string_is_not_an_annotation_but_one_after_code_is() {
    let inside = concat!("let code = \"// veri", "fies: REQ-A-0001.S1\";");
    assert!(scan(inside).is_empty(), "{inside}");
    let after = concat!("let u = \"http://x\"; // spe", "c: REQ-A-0001");
    assert_eq!(scan(after).len(), 1);
    let quote = concat!("let q = '\"'; // spe", "c: REQ-A-0001");
    assert_eq!(scan(quote).len(), 1);
}

// verifies: REQ-TRACE-0002.S1
#[test]
fn coverage_counts_implemented_requirements_and_verified_scenarios() {
    let g = graph();
    let spec = concat!("// spe", "c: REQ-A-0001");
    let one = concat!("// veri", "fies: REQ-A-0001.S1");
    let annotations = scan_text(Path::new("x.rs"), &format!("{spec}\n{one}\n"));
    let c = coverage(&g, &annotations, None);
    assert_eq!((c.requirements, c.implemented), (1, 1));
    assert_eq!((c.scenarios, c.verified), (2, 1));
    assert_eq!(c.unverified.len(), 1);
    assert!(c.unverified[0].ends_with(".S2"));
}

// verifies: REQ-TRACE-0002.S2
#[test]
fn a_reference_to_nothing_is_an_error_with_a_place() {
    let g = graph();
    let gone = concat!("// spe", "c: REQ-GONE-0001");
    let no_scenario = concat!("// veri", "fies: REQ-A-0001.S9");
    let annotations = scan_text(Path::new("src/x.rs"), &format!("\n{gone}\n{no_scenario}\n"));
    let c = coverage(&g, &annotations, None);
    let codes: Vec<&str> = c.findings.iter().map(|f| f.code).collect();
    assert_eq!(codes, ["E014", "E015"]);
    assert_eq!(c.findings[0].line, Some(2));
}
