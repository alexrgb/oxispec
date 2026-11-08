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

fn show(title: &str, context: &oxispec_core::context::Pack) {
    println!(
        "{title}: {} of {} tokens used",
        context.used, context.budget
    );
    for item in &context.items {
        let how = item.connection.as_deref().unwrap_or("asked for");
        println!(
            "  included {} ({how}), {} tokens{}",
            item.id,
            item.tokens,
            if item.truncated { ", text cut" } else { "" }
        );
    }
    for p in &context.pointers {
        println!("  only a pointer: {} ({} hops away)", p.id, p.distance);
    }
}

// spec: REQ-LIBRARY-0001
fn main() {
    println!("The graph: REQ-AUTH-0001 -decided-by-> ADR-0001 <-decided-by- REQ-PAY-0001\n");
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
    let seeds = vec!["REQ-AUTH-0001".to_string()];

    println!("CALL 1. Task about REQ-AUTH-0001: one hop of neighbors, a budget of 200 tokens.");
    show("RESULT 1", &pack(&graph, &seeds, 1, 200, &text_of));
    println!(
        "  The requirement and the decision behind it fit. REQ-PAY-0001 is two hops away, so it is left out.\n"
    );

    println!("CALL 2. The same task, two hops, a budget of 130 tokens.");
    show("RESULT 2", &pack(&graph, &seeds, 2, 130, &text_of));
    println!(
        "  The budget is the limit: what does not fit is named, so the agent can ask for it.\n"
    );

    println!("The text an agent would read for CALL 1:");
    let first = pack(&graph, &seeds, 1, 200, &text_of);
    for item in &first.items {
        println!("--- {} ---\n{}", item.id, item.text.trim());
    }
}
