//! Describe an API in requirements, and find an operation by what it does.
//!
//! A requirement titled `METHOD /path` is an operation. The index holds one
//! chunk of text for each. `HashEmbedder` needs no model and no network. To use
//! a neural model, write a type that implements `Embedder` and calls a service.
//!
//! Run it with `cargo run -p oxispec-core --example search_the_api`.

use std::fs;

use oxispec_core::Graph;
use oxispec_core::api;
use oxispec_core::apiindex::{self, Embedder, HashEmbedder, Store};

const ORDERS: &str = "\
---
id: REQ-ORDERS-0001
type: requirement
status: accepted
---
### Requirement: GET /orders/{id}
The system SHALL return one order by its number.

#### Scenario: An unknown order {#S1}
- **WHEN** a client asks for an order that does not exist
- **THEN** the API responds with 404: no such order
";

const PAYMENTS: &str = "\
---
id: REQ-PAYMENTS-0001
type: requirement
status: accepted
---
### Requirement: POST /payments
The system SHALL charge a card for an order.

#### Scenario: A declined card {#S1}
- **WHEN** the bank declines the card
- **THEN** the API responds with 402: the card is declined
";

// spec: REQ-LIBRARY-0001
fn main() -> Result<(), Box<dyn std::error::Error>> {
    // `operations` reads the requirement files, so this example writes them to a folder.
    let root = std::env::temp_dir().join(format!("oxispec-example-{}", std::process::id()));
    fs::create_dir_all(root.join("specs"))?;
    fs::write(root.join("specs/orders.md"), ORDERS)?;
    fs::write(root.join("specs/payments.md"), PAYMENTS)?;
    let graph = Graph::load(&root);

    println!("STEP 1. Requirements titled `METHOD /path` are the operations of the API:");
    let operations = api::operations(&graph, &root)?;
    for op in &operations {
        println!(
            "  {} {}  from {}, answers {:?}",
            op.method.to_uppercase(),
            op.path,
            op.node,
            op.responses.keys().collect::<Vec<_>>()
        );
    }

    let embedder = HashEmbedder;
    let chunks = apiindex::chunks(&graph, &operations, None);
    println!(
        "\nSTEP 2. One chunk of text for each operation. This is what is embedded for the first one:"
    );
    if let Some(first) = chunks.first() {
        println!(
            "  ----\n  {}\n  ----",
            first.text.trim().replace('\n', "\n  ")
        );
    }

    println!("\nSTEP 3. Embed the chunks into an index (here with a lexical embedder, no model).");
    let mut store = Store::new(&embedder.id());
    let stats = apiindex::sync(&mut store, chunks.clone(), &embedder)?;
    println!(
        "  first sync:  {} added, {} unchanged",
        stats.added, stats.unchanged
    );
    let stats = apiindex::sync(&mut store, chunks, &embedder)?;
    println!(
        "  second sync: {} added, {} unchanged  (only a changed chunk is embedded again)",
        stats.added, stats.unchanged
    );

    println!(
        "\nSTEP 4. Search: the query is embedded the same way, and the nearest chunks come back."
    );
    for text in ["charge a card", "fetch an order by its number"] {
        println!("  query: \"{text}\"");
        let query = embedder.embed(&[text.to_string()])?.remove(0);
        for hit in apiindex::search(&store, &query, 2) {
            println!("    {:.2}  {}", hit.score, hit.entry.call);
        }
    }
    println!("\nThe built-in embedder counts shared words, so it does not know synonyms.");
    println!("A neural model behind the `Embedder` trait would.");

    fs::remove_dir_all(&root)?;
    Ok(())
}
