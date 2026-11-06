# OxiSpec user guide

How to use the `oxispec` binary. Start with the first page, then take the one that matches what you want to do.

| Page | For |
|---|---|
| [Getting started](getting-started.md) | Install, create a project, write a first requirement, run the checks |
| [Concepts](concepts.md) | Nodes, IDs, links, modules and what `check` looks at |
| [Writing requirements](requirements.md) | Requirements and scenarios, decisions, change sets, review |
| [Writing a PRD](writing-a-prd.md) | A product requirements document made from the nodes |
| [Describing an API](writing-an-api.md) | Operations as requirements, an OpenAPI skeleton, and a check of the real file |
| [Code and tests](trace-and-coverage.md) | Tie code and tests to requirements and see what is not covered |
| [Moving from another tool](importing.md) | OpenSpec, Spec Kit, Kiro, plain markdown, a PRD or an OpenAPI file |
| [Checks in CI](ci-checks.md) | Fail pull requests on new problems, with reports |
| [Releasing the specs](spec-releases.md) | Notes, a version, gates and a tag |
| [Working with AI agents](working-with-agents.md) | Skills, and what an agent can do with them |
| [MCP for clients](mcp-clients.md) | Give Claude Code, Cursor, Gemini or Copilot live access |
| [Command reference](../reference/commands.md) | Every command and option, written from the program |
| [Check codes](../reference/check-codes.md) | Every code `check` can report |
| [MCP tools](../reference/mcp-tools.md) | Every tool the server offers, with arguments |

When you need the detail of one feature there is a reference page for it in [docs](../): [format](../format.md), [trace](../trace.md), [profiles](../profiles.md), [CI](../ci.md), [the site](../site.md), [agents](../agents.md), [MCP](../mcp.md) and [releases](../releasing.md). `oxispec --help` and `oxispec <command> --help` list every command and option.
