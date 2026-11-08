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

fn main() {
    let graph = Graph::from_sources([(
        PathBuf::from("specs/pay/refunds.md"),
        BROKEN.as_bytes().to_vec(),
    )]);

    let findings = check(&graph);
    for f in &findings {
        let level = match f.severity {
            Severity::Error => "error",
            Severity::Warning => "warning",
            Severity::Info => "info",
        };
        // `code` is stable, so a program can match on it (`E005` and so on).
        println!("{level} {}: {}", f.code, f.message);
    }

    // A program that gates on the checks, as `oxispec check` does, exits 1 when
    // there is an error: `std::process::exit(1)`.
    let errors = findings
        .iter()
        .filter(|f| f.severity == Severity::Error)
        .count();
    println!("{} findings, {errors} errors", findings.len());
}
