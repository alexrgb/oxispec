//! Run the structural checks and read what they find.
//!
//! Run it with `cargo run -p oxispec-core --example check_specs`.

use std::path::PathBuf;

use oxispec_core::{Graph, Severity, check};

// A requirement with a link to a node that does not exist, and no SHALL.
const BROKEN: &str = "\
---
id: REQ-PAY-0001
type: requirement
status: accepted
links:
  decided-by: [ADR-0099]
---
### Requirement: Refunds
Refunds are possible.

#### Scenario: A refund {#S1}
- **WHEN** a customer asks for a refund
- **THEN** the money goes back
";

// spec: REQ-LIBRARY-0001
fn main() {
    println!("INPUT. One requirement, with two mistakes:");
    println!("  - it links `decided-by` to ADR-0099, which is not in the graph");
    println!("  - its text has no SHALL or MUST, so it states nothing that must hold\n");
    let graph = Graph::from_sources([(
        PathBuf::from("specs/pay/refunds.md"),
        BROKEN.as_bytes().to_vec(),
    )]);

    println!("CALL. check(&graph) returns a list of findings:\n");
    let findings = check(&graph);
    for f in &findings {
        let level = match f.severity {
            Severity::Error => "error",
            Severity::Warning => "warning",
            Severity::Info => "info",
        };
        let place = f
            .path
            .as_ref()
            .map(|p| p.display().to_string())
            .unwrap_or_default();
        // `code` is stable, so a program can match on it (`E002` and so on).
        println!(
            "{level} {} in {} ({place}): {}",
            f.code,
            f.id.as_deref().unwrap_or("-"),
            f.message
        );
    }

    // A program that gates on the checks, as `oxispec check` does, exits 1 when
    // there is an error: `std::process::exit(1)`.
    let errors = findings
        .iter()
        .filter(|f| f.severity == Severity::Error)
        .count();
    println!(
        "\nRESULT. {} findings, {errors} errors. The command would exit with 1.",
        findings.len()
    );
}
