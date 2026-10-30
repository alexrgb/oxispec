# OxiSpec

Agent-first spec engine in Rust. Specs, ADRs, test cases, standards, API contracts and diagrams are nodes in one linked graph, with history, consistency checks and an MCP server for AI agents.

**Status: early.** The design is written and the first iteration is under way. See [docs/design.md](docs/design.md) for the full design and [docs/iterations.md](docs/iterations.md) for the plan and what is built so far.

## Try it

```bash
cargo install --path crates/oxispec-cli
oxispec init --preset standard
oxispec new requirement auth "Admin session timeout"
oxispec new adr "Use Postgres"
oxispec link REQ-AUTH-0001 decided-by ADR-0001
oxispec check
oxispec check --changed   # only what differs from git HEAD, and what links to it
oxispec show ADR-0001
oxispec history REQ-AUTH-0001   # how it changed, from git
oxispec diff v1                 # what changed since a tag, branch or commit
oxispec tag v2                  # name this revision of the specs
oxispec new change "Tighten timeouts"   # a proposal, with a lifecycle and an approval
oxispec rollback v1 REQ-AUTH-0001 --dry-run   # put a node back, after checking
oxispec ci --base origin/main            # only the findings this branch introduces
oxispec search session timeout          # full-text search, with a derived index
oxispec site --out public --releases    # a static site, with a page per node and per version
oxispec coverage                        # requirements with no code, scenarios with no test
oxispec suspect                         # code or specs that changed since a person last confirmed the link
oxispec results import junit.xml        # then: oxispec coverage --require-pass
oxispec release prepare                 # notes and a suggested version since the last release
oxispec release publish 0.2.0           # gates, then the tag specs-v0.2.0 (needs the releases module)
oxispec agents update                   # skills for Claude Code, Cursor, Copilot and Gemini CLI
oxispec context REQ-AUTH-0001 --budget 4000   # the node and what is near it, sized for an agent
oxispec mcp                             # the specs as an MCP server, for AI clients (needs the mcp module)
oxispec new vocabulary "Auth words"     # typed variables for claims (needs the consistency module)
oxispec claims                          # the vocabulary and the claims; `check` reports conflicts
```

Use `oxispec init --id-scheme random` when several branches add specs at the same time. The file format, link kinds, check codes and the local index are in [docs/format.md](docs/format.md). History, diffs, rollback and tags are in [docs/history.md](docs/history.md). Change sets and releases are described in [docs/format.md](docs/format.md#change-sets). Running it in CI is in [docs/ci.md](docs/ci.md), search is in [docs/search.md](docs/search.md), the static site is in [docs/site.md](docs/site.md), trace and coverage are in [docs/trace.md](docs/trace.md), agent skills are in [docs/agents.md](docs/agents.md), the MCP server and context packs are in [docs/mcp.md](docs/mcp.md), and claims and conflict checking are in [docs/claims.md](docs/claims.md).

`oxispec check` reports dangling links (including links to scenarios that do not exist), duplicate IDs, supersede cycles, requirements without a scenario or a SHALL/MUST, and requirements whose claims contradict each other. It exits with code 1 when there are errors.

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md). Work is planned in iterations, one branch and one pull request each. Version numbers follow [docs/versioning.md](docs/versioning.md). To report a security problem, see [SECURITY.md](SECURITY.md).

## Acknowledgements

OxiSpec is inspired by [OpenSpec](https://github.com/Fission-AI/OpenSpec) (MIT). It shares the idea of requirements with scenarios and delta changes, and extends it with a typed graph, history, consistency checking and hosted access control.

## License

Licensed under either of [Apache License 2.0](LICENSE-APACHE) or [MIT license](LICENSE-MIT), at your option.
