# Working with AI agents

OxiSpec gives an agent two things: **skills**, which tell it how to work with the specs, and the **MCP server**, which lets it search and read them. Neither lets it approve anything: a person approves change sets.

## Skills

`oxispec init` writes the skills into the folders of the clients you use (Claude Code `.claude/skills`, Cursor `.cursor/skills`, Copilot `.github/skills`, Gemini CLI `.gemini/skills`, and `.agents/skills` for any client that reads it):

```bash
oxispec agents list            # clients and skills, and which this project gets
oxispec agents update          # write or refresh the skills; a file edited by hand is left alone
oxispec agents check           # exit 1 when a skill is missing, out of date or edited by hand
```

Run `agents update` after upgrading `oxispec`, since the skills change with it, and `agents check` in CI to catch drift. The skills cover exploring the specs, proposing a change, applying and verifying it, syncing code, writing an ADR, managing debt, reviewing a change, rolling back and importing from another tool. A skill is only offered if the module it needs is on. To pick the clients, set `agents:` in the config or pass `--agents claude,cursor` to `init`. For tools that read skills in another layout (`ruler`, `rulesync`, a skills repository), use `oxispec agents export`.

## What an agent can do with them

Ask the agent in plain words. With the skills loaded, "propose a change that makes admin sessions expire after 10 minutes" makes it search for the requirement, write a draft change set linked to it, edit the node, run `oxispec check` and report. "Review this branch" makes it run `oxispec ci`, read the findings and look at what the change touches. It leaves the change set in `draft` or `in-review` for a person to approve.

## Context for an agent

```bash
oxispec context REQ-AUTH-0001 --budget 4000
```

prints the node and what is near it, cut to a token budget, which is what to paste into a prompt when the agent has no MCP.

## MCP

See [MCP for clients](mcp-clients.md) to give the agent live access to the specs.
