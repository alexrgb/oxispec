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

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // `operations` reads the requirement files, so this example writes them to a folder.
    let root = std::env::temp_dir().join(format!("oxispec-example-{}", std::process::id()));
    fs::create_dir_all(root.join("specs"))?;
    fs::write(root.join("specs/orders.md"), ORDERS)?;
    fs::write(root.join("specs/payments.md"), PAYMENTS)?;
    let graph = Graph::load(&root);

    let operations = api::operations(&graph, &root)?;
    for op in &operations {
        println!("{} {}  ({})", op.method.to_uppercase(), op.path, op.node);
    }

    // Embed one chunk for each operation. A second `sync` embeds only what changed.
    let embedder = HashEmbedder;
    let mut store = Store::new(&embedder.id());
    let chunks = apiindex::chunks(&graph, &operations, None);
    let stats = apiindex::sync(&mut store, chunks, &embedder)?;
    println!("{} added, {} unchanged", stats.added, stats.unchanged);

    // A query is embedded with the same embedder, and the nearest chunks come back.
    let query = embedder.embed(&["charge a card".to_string()])?.remove(0);
    for hit in apiindex::search(&store, &query, 2) {
        println!("{:.2}  {}", hit.score, hit.entry.call);
    }

    fs::remove_dir_all(&root)?;
    Ok(())
}
