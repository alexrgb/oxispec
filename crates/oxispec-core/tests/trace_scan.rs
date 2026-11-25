//! Tests of the trace scan that carry real `verifies:` comments. For a
//! language with a parser (Rust, Go, TypeScript, JavaScript) the scan reads a
//! marker only in a real comment, so the sample code in these strings is not
//! read as annotations.

use std::path::{Path, PathBuf};

use oxispec_core::graph::Graph;
use oxispec_core::trace::{Kind, coverage, scan_text};

fn scan_in(path: &str, text: &str) -> Vec<(String, Kind, usize)> {
    scan_text(Path::new(path), text)
        .into_iter()
        .map(|a| (a.target, a.kind, a.line))
        .collect()
}

fn scan(text: &str) -> Vec<(String, Kind, usize)> {
    scan_in("x.rs", text)
}

const REQ: &str = "---\nid: REQ-A-0001\ntype: requirement\nstatus: accepted\n---\n### Requirement: X\nThe system SHALL x.\n\n#### Scenario: One {#S1}\n- **WHEN** a\n- **THEN** b\n\n#### Scenario: Two {#S2}\n- **WHEN** c\n- **THEN** d\n";

fn graph() -> Graph {
    Graph::from_sources([(PathBuf::from("specs/a.md"), REQ.as_bytes().to_vec())])
}

// verifies: REQ-TRACE-0001.S1
#[test]
fn annotations_are_read_from_the_comments_of_many_languages() {
    let found = scan("// spec: REQ-A-0001\n/// spec: REQ-A-0002, ADR-0007\n");
    let targets: Vec<&str> = found.iter().map(|f| f.0.as_str()).collect();
    assert_eq!(targets, ["REQ-A-0001", "REQ-A-0002", "ADR-0007"]);
    assert_eq!(found[0].1, Kind::Implements);
    // Languages without a parser are read by their comment leaders.
    let py = scan_in("a.py", "# verifies: REQ-A-0001.S1\n");
    assert_eq!(
        (py[0].0.as_str(), py[0].1),
        ("REQ-A-0001.S1", Kind::Verifies)
    );
    assert_eq!(scan_in("a.sql", "-- spec: REQ-A-0003\n").len(), 1);
    assert_eq!(
        scan_in(
            "a.ts",
            "it('ends idle sessions [REQ-A-0001.S1]', () => {});\n"
        )
        .len(),
        1
    );
}

// verifies: REQ-TRACE-0001.S2
#[test]
fn prose_and_lookalikes_are_not_annotations() {
    for text in [
        "let spec: Spec = load();",
        "// the spec: see-above for details",
        "// spec: do-it",
        "// myspec: REQ-A-0001",
        "// spec:",
        "// see https://example.com/a:REQ-A-0001",
        "//! A `// spec: REQ-A-0001` comment sits above a function.",
        "//! // spec: REQ-A-0001",
    ] {
        assert!(scan(text).is_empty(), "{text}");
    }
}

// verifies: REQ-TRACE-0003.S1
#[test]
fn a_marker_in_a_string_is_not_an_annotation_but_one_in_a_comment_is() {
    for text in [
        "let code = \"// verifies: REQ-A-0001.S1\";",
        "let raw = r#\"\n// spec: REQ-A-0001\n\"#;",
        "let many = \"first\n// spec: REQ-A-0001\nlast\";",
        "assert_eq!(s[\"REQ-A-0001.S1\"], 1);",
        "// a test is named `ends idle sessions [REQ-A-0001.S1]`",
    ] {
        assert!(scan(text).is_empty(), "{text}");
    }
    assert_eq!(scan("let u = \"http://x\"; // spec: REQ-A-0001").len(), 1);
    assert_eq!(scan("let q = '\"'; // spec: REQ-A-0001").len(), 1);
    assert_eq!(scan("/* spec: REQ-A-0001 */ fn f() {}").len(), 1);
    assert_eq!(
        scan("/*\n * verifies: REQ-A-0001.S1\n */\nfn f() {}").len(),
        1
    );
}

// verifies: REQ-TRACE-0002.S1
#[test]
fn coverage_counts_implemented_requirements_and_verified_scenarios() {
    let g = graph();
    let annotations = scan_text(
        Path::new("x.rs"),
        "// spec: REQ-A-0001\n// verifies: REQ-A-0001.S1\n",
    );
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
    let annotations = scan_text(
        Path::new("src/x.rs"),
        "\n// spec: REQ-GONE-0001\n// verifies: REQ-A-0001.S9\n",
    );
    let c = coverage(&g, &annotations, None);
    let codes: Vec<&str> = c.findings.iter().map(|f| f.code).collect();
    assert_eq!(codes, ["E014", "E015"]);
    assert_eq!(c.findings[0].line, Some(2));
}

// verifies: REQ-TRACE-0007.S1
#[test]
fn a_test_title_matching_trace_test_ids_verifies_the_node_with_that_id() {
    let config: oxispec_core::config::TraceConfig =
        serde_saphyr::from_str("test_ids: ['TC-[A-Z]+-[0-9]+']").unwrap();
    let languages = oxispec_core::languages::Languages::from_config(&config).unwrap();
    let code = "describe('x', () => {\n  it('TC-CAT-001 lists programs', () => {});\n  it('plain', () => {});\n});\n";
    let found = oxispec_core::trace::scan_text_with(Path::new("a.test.ts"), code, &languages);
    assert_eq!(found.len(), 1, "{found:?}");
    assert_eq!(found[0].target, "TC-CAT-001");
    assert_eq!(found[0].kind, Kind::Verifies);
    assert_eq!(found[0].line, 2);
    // Without the setting nothing is read from the title.
    assert!(scan_in("a.test.ts", code).is_empty());
}

// verifies: REQ-TRACE-0007.S2
#[test]
fn a_pattern_match_without_a_node_is_left_out_and_a_bad_pattern_is_refused() {
    let config: oxispec_core::config::TraceConfig =
        serde_saphyr::from_str("test_ids: ['TC-[A-Z]+-[0-9]+']").unwrap();
    let languages = oxispec_core::languages::Languages::from_config(&config).unwrap();
    let found = oxispec_core::trace::scan_text_with(
        Path::new("a.test.ts"),
        "it('TC-CAT-001 x', () => {});\n",
        &languages,
    );
    let coverage = coverage(&graph(), &found, None);
    assert!(coverage.findings.is_empty(), "{:?}", coverage.findings);
    let bad: oxispec_core::config::TraceConfig =
        serde_saphyr::from_str("test_ids: ['TC-[']").unwrap();
    let err = oxispec_core::languages::Languages::from_config(&bad).unwrap_err();
    assert!(err.contains("trace.test_ids"), "{err}");
}
