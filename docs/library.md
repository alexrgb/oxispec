# Using the library

`oxispec-core` is the library the `oxispec` command is built on. Use it to read specifications in your own program, to check them, to tie code to them, or to give an AI agent the right text.

The API is not stable before 1.0. A minor version may change it, and the changes are in the changelog. The crate is not on crates.io yet; use it from a path or a git dependency until it is.

```toml
[dependencies]
oxispec-core = { git = "https://github.com/alexrgb/oxispec" }
```

## The idea

A specification is a markdown file with a front matter. A `Graph` holds all of them as `Node`s, with typed links between them. Almost everything else is a function that takes a graph and returns what it found.

There are two ways to get a graph:

- `Graph::load(root)` reads a project from disk.
- `Graph::from_sources(...)` takes `(path, bytes)` pairs, so the files can come from git, from a database, or from a string in a test.

## The examples

Each one runs on its own, with no project on disk, and a test runs them all, so they do not go stale.

| Example | What it shows | Run |
|---|---|---|
| [read_specs](../crates/oxispec-core/examples/read_specs.rs) | Build a graph, list nodes, links and scenarios, and find what links to a node | `cargo run -p oxispec-core --example read_specs` |
| [check_specs](../crates/oxispec-core/examples/check_specs.rs) | Run the checks and read the findings, each with a stable code | `cargo run -p oxispec-core --example check_specs` |
| [trace_coverage](../crates/oxispec-core/examples/trace_coverage.rs) | Read `spec:` and `verifies:` comments and count what is implemented and verified | `cargo run -p oxispec-core --example trace_coverage` |
| [context_for_an_agent](../crates/oxispec-core/examples/context_for_an_agent.rs) | Pack the nodes for a task into a budget of tokens, nearest first | `cargo run -p oxispec-core --example context_for_an_agent` |
| [compare_revisions](../crates/oxispec-core/examples/compare_revisions.rs) | Compare two graphs: what was added, removed and changed, and which findings appeared | `cargo run -p oxispec-core --example compare_revisions` |
| [search_the_api](../crates/oxispec-core/examples/search_the_api.rs) | Read an API from requirements and find an operation by what it does | `cargo run -p oxispec-core --example search_the_api` |

## Points to know

- **Findings have codes.** `Finding::code` (`E002`, `W017`) does not change from one run to the next, so a program can match on it. The messages are for people and may be reworded. The codes are listed in [the reference](reference/check-codes.md).
- **Reading code.** `trace::scan_text` reads one file and `trace::scan_tree` a whole project. For Rust, Go, TypeScript and JavaScript a marker counts only in a real comment. See [trace.md](trace.md).
- **Your own embedder.** `apiindex::Embedder` has two methods, `id` and `embed`. `HashEmbedder` has no model. To use a neural model, write a type that calls your service and give it to `apiindex::sync`. The command reads such a service from the config (see the [API guide](guide/writing-an-api.md#search-the-operations)).
- **Reading is cheap, writing is not here.** The library reads and checks. To create and edit nodes, `edit::add_link` and `edit::set_fields` change the text of a node and give it back; writing the file is yours.

## Not covered

The examples do not show the claims, state machine and architecture analyses, release gates, the site, or importing. Their modules (`claims`, `machines`, `architecture`, `release`, `site`, `import`) have the same shape: a function from a graph to a result. The command in `crates/oxispec-cli` shows how each one is called.
