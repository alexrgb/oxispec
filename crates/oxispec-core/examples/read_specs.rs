//! Read specifications into a graph and walk it.
//!
//! The graph is built from text here, so the example needs no files. To read a
//! project from disk, use `Graph::load(root)`.
//!
//! Run it with `cargo run -p oxispec-core --example read_specs`.

use std::path::PathBuf;

use oxispec_core::Graph;

const REQUIREMENT: &str = "\
---
id: REQ-AUTH-0001
type: requirement
status: accepted
links:
  decided-by: [ADR-0001]
---
### Requirement: Admin session timeout
The system SHALL end an admin session after 15 minutes without activity.

#### Scenario: Idle admin {#S1}
- **WHEN** an admin does nothing for 15 minutes
- **THEN** the session ends
";

const DECISION: &str = "\
---
id: ADR-0001
type: adr
status: accepted
---
# Keep sessions on the server
Sessions are stored on the server, so that they can be ended from there.
";

// spec: REQ-LIBRARY-0001
fn main() {
    println!("STEP 1. Build a graph from two files (given as text, not read from disk).");
    // Paths are relative to the project root. Bytes are what a file holds.
    let graph = Graph::from_sources([
        (
            PathBuf::from("specs/auth/timeout.md"),
            REQUIREMENT.as_bytes().to_vec(),
        ),
        (
            PathBuf::from("specs/decisions/0001.md"),
            DECISION.as_bytes().to_vec(),
        ),
    ]);
    println!("The graph has {} nodes.\n", graph.nodes.len());

    println!("STEP 2. Walk the nodes: what each one is, what it links to, what it says.");
    for node in &graph.nodes {
        // A decision has a heading. A requirement is named by its blocks, listed below.
        match node.title.as_deref() {
            Some(title) => println!("{}  (type {}, \"{title}\")", node.id, node.ty),
            None => println!("{}  (type {})", node.id, node.ty),
        }
        println!("    file: {}", node.path.display());
        for (kind, targets) in &node.links {
            println!("    link: {kind} -> {}", targets.join(", "));
        }
        for block in &node.requirements {
            println!("    requirement block: {}", block.name);
            for scenario in &block.scenarios {
                println!("        scenario: {}", scenario.name);
            }
        }
    }

    println!("\nSTEP 3. Ask what links TO a node. The file of ADR-0001 says nothing about this:");
    println!("a link is written on one node only, and the other side is worked out.");
    for (from, kind) in graph.incoming("ADR-0001") {
        println!("{} points at ADR-0001 with `{kind}`", from.id);
    }
}
