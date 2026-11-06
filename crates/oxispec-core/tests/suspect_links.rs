//! Tests of suspect links that carry real `verifies:` comments. The sample
//! code is in strings, which the scan does not read as comments.

use std::path::Path;

use oxispec_core::graph::Graph;
use oxispec_core::parse::parse_file_text;
use oxispec_core::suspect::{Lock, State, confirm, links};
use oxispec_core::trace::scan_text;

fn graph(body: &str) -> Graph {
    let text = format!(
        "---\nid: REQ-A-0001\ntype: requirement\n---\n### Requirement: A\n{body}\n#### Scenario: S {{#S1}}\n- **WHEN** a\n- **THEN** b\n"
    );
    Graph::from_nodes(
        parse_file_text(Path::new("a.md"), &text)
            .0
            .into_iter()
            .collect(),
    )
}

fn code(body: &str, tail: &str) -> String {
    format!("{}{body}{tail}", "// spec: REQ-A-0001\n")
}

fn states(g: &Graph, code: &str, lock: &Lock) -> Vec<State> {
    let found = scan_text(Path::new("src/a.rs"), code);
    links(g, &found, lock).iter().map(|l| l.state).collect()
}

fn confirmed(g: &Graph, code: &str) -> Lock {
    let mut lock = Lock::default();
    let found = scan_text(Path::new("src/a.rs"), code);
    confirm(g, &found, &mut lock, None);
    lock
}

// verifies: REQ-SUSPECT-0001.S1
#[test]
fn a_link_is_unconfirmed_then_ok_and_formatting_is_not_a_change() {
    let g = graph("The system SHALL end idle sessions.");
    let base = code("fn expire() {\n    work(15);\n}\n", "");
    assert_eq!(states(&g, &base, &Lock::default()), [State::Unconfirmed]);
    let lock = confirmed(&g, &base);
    assert_eq!(states(&g, &base, &lock), [State::Ok]);
    let tidy = code("fn expire() {\n\n        work(15); // minutes\n}\n", "");
    assert_eq!(states(&g, &tidy, &lock), [State::Ok]);
}

// verifies: REQ-SUSPECT-0001.S2
#[test]
fn a_change_on_either_side_makes_the_link_suspect() {
    let g = graph("The system SHALL end idle sessions.");
    let base = code("fn expire() {\n    work(15);\n}\n", "");
    let lock = confirmed(&g, &base);
    let changed = code("fn expire() {\n    work(30);\n}\n", "");
    assert_eq!(states(&g, &changed, &lock), [State::SuspectCode]);
    let g2 = graph("The system SHALL end idle sessions after a timeout.");
    assert_eq!(states(&g2, &base, &lock), [State::SuspectSpec]);
    assert_eq!(states(&g2, &changed, &lock), [State::SuspectBoth]);
    assert!(State::SuspectBoth.is_suspect());
    assert!(!State::Ok.is_suspect());
}

// verifies: REQ-SUSPECT-0001.S3
#[test]
fn a_removed_annotation_is_orphaned_and_confirming_again_prunes_it() {
    let g = graph("The system SHALL end idle sessions.");
    let base = code("fn expire() {\n    work(15);\n}\n", "");
    let mut lock = confirmed(&g, &base);
    let none = "fn expire() {\n    work(15);\n}\n";
    assert_eq!(
        links(&g, &scan_text(Path::new("src/a.rs"), none), &lock).len(),
        1
    );
    assert_eq!(
        links(&g, &scan_text(Path::new("src/a.rs"), none), &lock)[0].state,
        State::Orphaned
    );
    let found = scan_text(Path::new("src/a.rs"), none);
    confirm(&g, &found, &mut lock, None);
    assert!(links(&g, &found, &lock).is_empty());
}
