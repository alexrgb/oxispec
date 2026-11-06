# MCP for clients

`oxispec mcp` serves the specs over the Model Context Protocol, so an AI client can search them, read a node with its links, run the checks and propose a change, without you pasting files. It needs the `mcp` module (included in the `standard` preset).

## Connect a client

From the project folder:

```bash
oxispec connect --dry-run        # say what would be written
oxispec connect                  # all four clients, or those in `agents` in the config
oxispec connect claude cursor    # only these
oxispec connect --read-only      # leave out the tool that writes
```

```text
would create .mcp.json (Claude Code)
```

It adds or replaces only the `oxispec` entry in `.mcp.json` (Claude Code), `.cursor/mcp.json` (Cursor), `.gemini/settings.json` (Gemini CLI) and `.vscode/mcp.json` (VS Code with Copilot). Other settings stay, and running it twice changes nothing. If a file is not plain JSON, it stops and tells you to add the entry by hand. Then restart the client.

The entry starts `oxispec mcp` as the client needs it, in the project folder. The file names and shapes are from each client's documentation; they have not been run against every client here.

## What the client gets

Read tools: `spec_search`, `spec_get`, `spec_neighbors`, `spec_context`, `spec_check`, `spec_status`, `spec_history`, `spec_diff`, `spec_trace`, and, when their modules are on, `spec_debt`, `spec_risks`, `spec_architecture`, `spec_machine` and `spec_claims`. They give the same answers as the commands with `--json`. One tool writes: `spec_propose_change` creates a draft change set. It cannot approve, and nothing is committed. `--read-only` removes it. Every tool with its arguments is in the [MCP tools reference](../reference/mcp-tools.md).

## One server for a team

To serve many clients from one place, run a read-only server over HTTP with a token for each client:

```bash
oxispec token add ci               # prints the token once
oxispec serve                      # listens on 127.0.0.1:7878 by default
```

and point the clients at it:

```bash
OXISPEC_TOKEN=... oxispec connect --url https://specs.example.com/mcp
```

The token is read from the environment variable `OXISPEC_TOKEN` and is never written to a file. A served project is always read-only. Put it behind HTTPS (a TLS proxy in front, with `--behind-tls-proxy` when it listens on a non-local address); plain `http://` to another machine prints a warning. Tokens, limits and the audit log are in [MCP](../mcp.md#serving-many-clients-over-http).
