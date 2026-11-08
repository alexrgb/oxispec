//! Tie code and tests to requirements with comments, and count what is covered.
//!
//! A `spec:` comment says what a piece of code implements. A `verifies:`
//! comment says what scenario a test checks.
//!
//! Run it with `cargo run -p oxispec-core --example trace_coverage`.

use std::path::{Path, PathBuf};

use oxispec_core::Graph;
use oxispec_core::trace::{coverage, scan_text};

const REQUIREMENT: &str = "\
---
id: REQ-AUTH-0001
type: requirement
status: accepted
---
### Requirement: Admin session timeout
The system SHALL end an admin session after 15 minutes without activity.

#### Scenario: Idle admin {#S1}
- **WHEN** an admin does nothing for 15 minutes
- **THEN** the session ends

#### Scenario: Active admin {#S2}
- **WHEN** an admin keeps working
- **THEN** the session stays open
";

const CODE: &str = "\
// spec: REQ-AUTH-0001
fn end_idle_sessions() {}

// verifies: REQ-AUTH-0001.S1
#[test]
fn an_idle_admin_is_signed_out() {}
";

fn main() {
    let graph = Graph::from_sources([(
        PathBuf::from("specs/auth.md"),
        REQUIREMENT.as_bytes().to_vec(),
    )]);

    // For a language with a parser (Rust, Go, TypeScript, JavaScript) a marker counts
    // only in a real comment. `scan_tree` does the same for every file of a project.
    let annotations = scan_text(Path::new("src/session.rs"), CODE);
    for a in &annotations {
        println!("{:?} {} at line {}", a.kind, a.target, a.line);
    }

    // `None` counts requirements in every status. Pass `Some("accepted")` to leave drafts out.
    let report = coverage(&graph, &annotations, None);
    println!(
        "implemented {}/{} ({:.0}%), verified {}/{} scenarios ({:.0}%)",
        report.implemented,
        report.requirements,
        report.implemented_percent(),
        report.verified,
        report.scenarios,
        report.verified_percent(),
    );
    for id in &report.unverified {
        println!("no test verifies {id}");
    }
}
