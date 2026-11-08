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

fn statement(graph: &Graph, id: &str) -> String {
    graph
        .get(id)
        .and_then(|n| n.requirements.first())
        .map(|b| b.statement.clone())
        .unwrap_or_default()
}

// spec: REQ-LIBRARY-0001
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
    println!(
        "BEFORE: REQ-A-0001 and REQ-A-0002.  AFTER: REQ-A-0001 (reworded) and REQ-A-0003 (new).\n"
    );

    println!("CALL. diff_graphs(&before, &after):\n");
    let diff = diff_graphs(&before, &after);
    for n in &diff.added {
        println!("added   {}: {}", n.id, statement(&after, &n.id));
    }
    for n in &diff.removed {
        println!("removed {}: {}", n.id, statement(&before, &n.id));
    }
    for n in &diff.changed {
        println!("changed {}", n.id);
        println!("    was: {}", statement(&before, &n.id));
        println!("    now: {}", statement(&after, &n.id));
        for change in &n.changes {
            println!("    the diff says: {}", change.describe());
        }
    }
    // Problems that the change brings in, and problems it settles.
    println!(
        "\nThe change brings {} new problems and settles {}.",
        diff.new_findings.len(),
        diff.fixed_findings.len()
    );
}
