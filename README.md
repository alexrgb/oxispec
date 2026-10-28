# OxiSpec

Agent-first spec engine in Rust. Specs, ADRs, test cases, standards, API contracts and diagrams are nodes in one linked graph, with history, consistency checks and an MCP server for AI agents.

**Status: early.** The design is written and the first iteration is under way. See [docs/design.md](docs/design.md) for the full design and [docs/iterations.md](docs/iterations.md) for the plan and what is built so far.

## Try it (once iteration 1 is merged)

```bash
cargo install --path crates/oxispec-cli
oxispec init --preset standard
oxispec new requirement auth "Admin session timeout"
oxispec new adr "Use Postgres"
oxispec check
oxispec show REQ-AUTH-0001
```

`oxispec check` reports dangling links, duplicate IDs, supersede cycles, and requirements without a scenario or a SHALL/MUST. It exits with code 1 when there are errors.

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md). Work is planned in iterations, one branch and one pull request each. Version numbers follow [docs/versioning.md](docs/versioning.md). To report a security problem, see [SECURITY.md](SECURITY.md).

## Acknowledgements

OxiSpec is inspired by [OpenSpec](https://github.com/Fission-AI/OpenSpec) (MIT). It shares the idea of requirements with scenarios and delta changes, and extends it with a typed graph, history, consistency checking and hosted access control.

## License

Licensed under either of [Apache License 2.0](LICENSE-APACHE) or [MIT license](LICENSE-MIT), at your option.
