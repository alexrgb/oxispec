# MCP server and context packs

`oxispec mcp` serves the specs to AI clients over the [Model Context Protocol](https://modelcontextprotocol.io), on stdin and stdout. It needs the `mcp` module (`oxispec modules add mcp`, included in the `standard` preset). The client starts it, so there is nothing to host.

The server speaks protocol versions 2025-06-18, 2025-03-26 and 2024-11-05, with one JSON-RPC message per line. It offers tools only. Resources and prompts are not offered yet.

## Connect a client

Clients start the server from the project folder, so `oxispec mcp` finds the project. Add `"--root", "/path/to/project"` to `args` if yours does not.

| Client | File | Content |
|---|---|---|
| Claude Code | `.mcp.json` | `{"mcpServers": {"oxispec": {"command": "oxispec", "args": ["mcp"]}}}` |
| Cursor | `.cursor/mcp.json` | the same |
| Gemini CLI | `.gemini/settings.json` | the same, under `mcpServers` next to your other settings |
| VS Code with Copilot | `.vscode/mcp.json` | `{"servers": {"oxispec": {"type": "stdio", "command": "oxispec", "args": ["mcp"]}}}` |

The file names and shapes are from each client's documentation. They are not written for you yet, since the files often hold other settings, and I have not run each client against the server.

Add `"--read-only"` to `args` to leave out the one tool that writes.

## Tools

| Tool | Does | Same as |
|---|---|---|
| `spec_search` | Full-text search: IDs, titles, snippets | `search --json` |
| `spec_get` | One node with its **text**, links and what links to it, optionally `at` a revision | `show --json`, plus the text |
| `spec_neighbors` | Nodes within 1 to 4 link hops, both directions, optionally only some link kinds | |
| `spec_context` | A context pack (below) | `context` |
| `spec_check` | The structural checks, optionally `at` a revision | `check --json` |
| `spec_history` | How a node changed | `history --json` |
| `spec_diff` | How the specs differ between two revisions | `diff --json` |
| `spec_trace` | Code and tests for a node (needs `tests`) | `trace --json` |
| `spec_propose_change` | Writes a draft change set (needs `specs`, not offered with `--read-only`) | `new change` and `link` |

Tool names use underscores, not dots, because some clients do not accept dots. The read tools run the same code as the `--json` commands, so an agent and a person get the same answers, and the JSON has the same `schema_version`.

**Proposing.** `spec_propose_change` takes a title, an optional reason, and the IDs of the nodes the change modifies. It checks that every ID exists before it writes anything. It creates a change set with status `draft` and links it with `changes`. It cannot approve: the server has no tool for `change status`, and the instructions it sends say that a person approves. The agent edits the nodes themselves in the files, as a person would. Nothing is committed.

**Arguments are checked.** IDs must be valid IDs, revisions cannot start with `-`, and numbers have limits, so an argument cannot be read as an option by git or by `oxispec`.

**An error is a result.** A tool that fails returns `isError: true` and a message the agent can read. A malformed message gets a JSON-RPC error.

## Context packs

```bash
oxispec context REQ-AUTH-0001                        # the node, and what is within 2 hops, in 8000 tokens
oxispec context REQ-AUTH-0001 ADR-0003 --depth 1 --budget 3000
oxispec context REQ-AUTH-0001 --at v1 --json
```

A pack is the text of the nodes you ask for and the nodes near them, nearest first, until the budget is used. Tokens are counted as one per four characters, which is a rough size for English and code, not a tokenizer's count.

- **Order.** By link hops, then by type (requirements, ADRs, standards, decisions, others, change sets), then by ID. A link counts in both directions, so a pack for an ADR includes the requirements it decides.
- **Fit.** A node whose text does not fit is skipped, and a smaller one after it can still go in. A node you asked for is cut to fit rather than dropped, with a note.
- **Pointers.** What did not fit is listed by ID, type and title under "Related, not included", up to 50, so the agent knows it exists and can ask for it.
- Each node says how it is connected, for example `REQ-AUTH-0001 decided-by ADR-0001`.

This is ranking by graph distance only. Ranking by similarity, scopes (`.oxispec/scope.yaml`), the digest, and filtering by access rules come later, with those features.
