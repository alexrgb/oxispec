# MCP server and context packs

`oxispec mcp` serves the specs to AI clients over the [Model Context Protocol](https://modelcontextprotocol.io), on stdin and stdout. It needs the `mcp` module (`oxispec modules add mcp`, included in the `standard` preset). The client starts it, so there is nothing to host.

The server speaks protocol versions 2025-06-18, 2025-03-26 and 2024-11-05, with one JSON-RPC message per line. It offers tools, node resources and three prompts.

## Connect a client

Clients start the server from the project folder, so `oxispec mcp` finds the project. Add `"--root", "/path/to/project"` to `args` if yours does not.

| Client | File | Content |
|---|---|---|
| Claude Code | `.mcp.json` | `{"mcpServers": {"oxispec": {"command": "oxispec", "args": ["mcp"]}}}` |
| Cursor | `.cursor/mcp.json` | the same |
| Gemini CLI | `.gemini/settings.json` | the same, under `mcpServers` next to your other settings |
| VS Code with Copilot | `.vscode/mcp.json` | `{"servers": {"oxispec": {"type": "stdio", "command": "oxispec", "args": ["mcp"]}}}` |

`oxispec connect` writes these entries for you:

```bash
oxispec connect                      # all four clients, or the ones in `agents` in the config
oxispec connect claude cursor        # only these
oxispec connect --read-only          # start the server with --read-only
oxispec connect --dry-run            # say what would change, write nothing
```

It adds or replaces only the `oxispec` entry. Other settings and servers stay, and running it twice changes nothing. Keys come back in alphabetical order. A file that is not plain JSON (VS Code allows comments in its file) is never rewritten: the command stops before writing anything and tells you to add the entry by hand. The file names and shapes are from each client's documentation; I have not run each client against the server.

Add `"--read-only"` to `args` to leave out the one tool that writes.

## Tools

| Tool | Does | Same as |
|---|---|---|
| `spec_search` | Full-text search: IDs, titles, snippets | `search --json` |
| `spec_get` | One node with its **text**, links and what links to it, optionally `at` a revision | `show --json`, plus the text |
| `spec_neighbors` | Nodes within 1 to 4 link hops, both directions, optionally only some link kinds | |
| `spec_context` | A context pack (below) | `context` |
| `spec_check` | The structural checks, optionally `at` a revision | `check --json` |
| `spec_status` | A summary: nodes by type and status, findings by code, change sets in flight, the latest release, optionally `at` a revision | `status --json` |
| `spec_history` | How a node changed | `history --json` |
| `spec_diff` | How the specs differ between two revisions | `diff --json` |
| `spec_trace` | Code and tests for a node (needs `tests`) | `trace --json` |
| `spec_architecture` | The architecture model, or with `view` and `id` one Mermaid view (needs `architecture`). Problems come from `spec_check` | `architecture --json`, `architecture view` |
| `spec_machine` | State machines: all of them, or one `id` with its properties, and with `view` set to `diagram` or `tests` a Mermaid diagram or one test per transition (needs `statemachines`). Problems come from `spec_check` | `machine --json` |
| `spec_claims` | The vocabulary and the claims on each variable, optionally one `variable` (needs `consistency`). Conflicts come from `spec_check` | `claims --json` |
| `spec_propose_change` | Writes a draft change set (needs `specs`, not offered with `--read-only`) | `new change` and `link` |

Tool names use underscores, not dots, because some clients do not accept dots. The read tools run the same code as the `--json` commands, so an agent and a person get the same answers, and the JSON has the same `schema_version`.

**Proposing.** `spec_propose_change` takes a title, an optional reason, and the IDs of the nodes the change modifies. It checks that every ID exists before it writes anything. It creates a change set with status `draft` and links it with `changes`. It cannot approve: the server has no tool for `change status`, and the instructions it sends say that a person approves. The agent edits the nodes themselves in the files, as a person would. Nothing is committed.

**Arguments are checked.** IDs must be valid IDs, revisions cannot start with `-`, and numbers have limits, so an argument cannot be read as an option by git or by `oxispec`.

**No process per call.** Tools run in the server's own process, with the same code as the commands. The server keeps the last graph, and the results of the checks, the claims, the state machines and the architecture model on it, in memory. A file watcher tells it when something under the project changed, so a call that finds nothing changed reads no files at all. Commands that write tell the server themselves, without waiting for the watcher. The open search index is kept too, so a repeated search needs no lock and no file work. Edits made while the server runs show up on the next call; the watcher reports in a few milliseconds, which is much less than the time an agent takes between calls, but a call made in the same instant as an edit by another program can still see the old files.

Two fallbacks keep it correct. If the watcher cannot start (for example, the system is out of watch handles), every call compares the files instead, as before. And every 30 seconds the files are compared in full anyway, in case an event was missed. A file changed in the last two seconds is never kept, because a second edit in the same clock tick could go unseen.

**An error is a result.** A tool that fails returns `isError: true` and a message the agent can read. A malformed message gets a JSON-RPC error.

## Serving many clients over HTTP

`oxispec mcp` starts one process per client. For a team, or an agent that opens many sessions, run one server instead:

```bash
oxispec token add ci                         # prints a token once; the project keeps a hash
oxispec token add docs-bot --types adr,requirement
oxispec serve                                # http://127.0.0.1:7878/mcp, read-only
```

One process holds the graph in memory and answers every client. A client sends each JSON-RPC message as a `POST` to `/mcp` with `Authorization: Bearer <token>` and gets one JSON answer; a notification gets `202`. There is no event stream and no session. The server is always read-only: it offers no `spec_propose_change`. `GET /health` answers `ok` without a token.

**Tokens.** `oxispec token add NAME` makes a token that starts with `oxs_`. Only its BLAKE3 hash is stored, in `.oxispec/server/tokens.json` (readable by the owner only, and added to `.oxispec/.gitignore`). `token list` shows names and scopes, never secrets. `token revoke NAME` takes effect on the next request, with no restart. The server will not start without a token.

**Limited tokens.** `--types adr,requirement` limits a token to nodes of those types, and to `spec_search` and `spec_get`. The other tools look at the whole project, so a limited token cannot call them; they look like tools that do not exist. A limited token follows these rules:

- Search narrows the index query itself, so hidden nodes are not ranked, and the index statistics are left out of the answer.
- A node of another type answers exactly like a node that does not exist.
- A link to a node of another type, in either direction, shows as `[restricted node]`.
- `resources/list` and `resources/read` show only the allowed types.

The text of an allowed node is shown as written, so a hidden ID quoted in its prose is not hidden. Only the node type is a limit for now; status, owner and classification are not.

**Audit log.** Every request is appended as one JSON line to `.oxispec/server/audit.jsonl` (or `--audit-log FILE`): time, token name, method, tool, node ID, and `ok`, `error`, `denied` or `unauthenticated`. Search words and other arguments are not recorded, and neither are tokens.

**Network.** The server listens on `127.0.0.1:7878` and accepts only requests addressed to `localhost`, which stops a web page from reaching it through a DNS name that points at your machine. It does not speak TLS, so it refuses any other address unless you pass `--behind-tls-proxy`, which says that a proxy in front encrypts the traffic and sets the `Host` header. Requests are limited to 1 MiB, and there is no rate limit yet.

Not built yet: logins through an identity provider (OIDC), policies beyond node types, and writes through the server. See the [design](design.md#17-access-control-rbac-plus-abac).

## Resources and prompts

**Resources.** Every node is a resource with the URI `oxispec://node/<ID>` and the MIME type `text/markdown`. `resources/list` returns them sorted by ID, 500 to a page with a `nextCursor`; `resources/templates/list` gives the template `oxispec://node/{id}`; `resources/read` returns the file as it is in the working tree. Clients that let a person attach a resource can attach a node this way. Only `oxispec://node/` URIs are read, and the ID is validated first. There are no subscriptions: the list is read when asked.

**Prompts.** Each takes one argument, `id`, and sends one user message that tells the agent which tools to call:

| Prompt | Asks the agent to |
|---|---|
| `review-requirement` | review a requirement for ambiguity, missing scenarios and conflicts, without editing |
| `implement-requirement` | write code and tests with `spec:` and `verifies:` comments, then run `spec_check` |
| `assess-impact` | list what changing a node would affect, without editing |

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
