//! Compare two states of the specs, as a review of a change does.
//!
//! Each state is a graph. Here they are written in the example, and in a
//! project they come from two git revisions or from a branch and its base.
//!
//! Run it with `cargo run -p oxispec-core --example compare_revisions`.

use std::path::PathBuf;

use oxispec_core::Graph;
use oxispec_core::diff::diff_graphs;

fn requirement(id: &str, shall: &str) -> (PathBuf, Vec<u8>) {
    let text = format!(
        "---\nid: {id}\ntype: requirement\nstatus: accepted\n---\n### Requirement: A rule\n{shall}\n\n#### Scenario: Case {{#S1}}\n- **WHEN** it happens\n- **THEN** it works\n"
    );
    (
        PathBuf::from(format!("specs/{}.md", id.to_lowercase())),
        text.into_bytes(),
    )
}

fn main() {
    let before = Graph::from_sources([
        requirement(
            "REQ-A-0001",
            "The system SHALL lock an account after 5 failed logins.",
        ),
        requirement("REQ-A-0002", "The system SHALL log every login."),
    ]);
    let after = Graph::from_sources([
        requirement(
            "REQ-A-0001",
            "The system SHALL lock an account after 3 failed logins.",
        ),
        requirement(
            "REQ-A-0003",
            "The system SHALL send a mail when an account is locked.",
        ),
    ]);

    let diff = diff_graphs(&before, &after);
    for n in &diff.added {
        println!("added   {}", n.id);
    }
    for n in &diff.removed {
        println!("removed {}", n.id);
    }
    for n in &diff.changed {
        println!("changed {}", n.id);
        for change in &n.changes {
            println!("    {}", change.describe());
        }
    }
    // Problems that the change brings in, and problems it settles.
    println!(
        "{} new findings, {} fixed",
        diff.new_findings.len(),
        diff.fixed_findings.len()
    );
}
