//! Pick the text an AI agent needs for a task, within a budget of tokens.
//!
//! The seeds come first, then the nodes linked to them, nearest first. What
//! does not fit is listed as a pointer, so that the agent knows it exists.
//!
//! Run it with `cargo run -p oxispec-core --example context_for_an_agent`.

use std::path::PathBuf;

use oxispec_core::Graph;
use oxispec_core::context::pack;

const FILES: [(&str, &str); 3] = [
    (
        "specs/auth.md",
        "---\nid: REQ-AUTH-0001\ntype: requirement\nstatus: accepted\nlinks:\n  decided-by: [ADR-0001]\n---\n### Requirement: Admin session timeout\nThe system SHALL end an admin session after 15 minutes without activity.\n\n#### Scenario: Idle admin {#S1}\n- **WHEN** an admin does nothing for 15 minutes\n- **THEN** the session ends\n",
    ),
    (
        "specs/adr.md",
        "---\nid: ADR-0001\ntype: adr\nstatus: accepted\n---\n# Keep sessions on the server\nSessions are stored on the server, so that they can be ended from there.\n",
    ),
    (
        "specs/other.md",
        "---\nid: REQ-PAY-0001\ntype: requirement\nstatus: accepted\nlinks:\n  decided-by: [ADR-0001]\n---\n### Requirement: Refunds\nThe system SHALL refund a payment within 5 days.\n\n#### Scenario: A refund {#S1}\n- **WHEN** a customer asks\n- **THEN** the money goes back\n",
    ),
];

fn main() {
    let sources = FILES
        .iter()
        .map(|(path, text)| (PathBuf::from(path), text.as_bytes().to_vec()));
    let graph = Graph::from_sources(sources);

    // The pack asks how to read a node's text, so it works for files on disk,
    // for text from git, or from anywhere else.
    let text_of = |node: &oxispec_core::Node| {
        FILES
            .iter()
            .find(|(path, _)| std::path::Path::new(path) == node.path)
            .map(|(_, text)| text.to_string())
    };

    // The nodes asked for, one hop of neighbors, and a budget of 200 tokens.
    let seeds = vec!["REQ-AUTH-0001".to_string()];
    let context = pack(&graph, &seeds, 1, 200, &text_of);

    println!("{} of {} tokens used", context.used, context.budget);
    for item in &context.items {
        let how = item.connection.as_deref().unwrap_or("asked for");
        println!(
            "--- {} ({how}){}",
            item.id,
            if item.truncated { " [cut]" } else { "" }
        );
        println!("{}", item.text.trim());
    }
    for p in &context.pointers {
        println!("not included: {} ({} hops)", p.id, p.distance);
    }
}
