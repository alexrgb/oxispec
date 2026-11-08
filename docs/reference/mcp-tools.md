# MCP tools

Written by `oxispec docs reference` from the server itself. Do not edit it by hand.

These are the tools `oxispec mcp` offers when every module is on. A tool whose module is off is not offered. How to connect a client, and how the server behaves, is in [MCP](../mcp.md).

## `spec_search`

Full-text search of every node. Returns IDs, titles and snippets, best first.

Read only.

Arguments:

- `query` (string, required): Words to find. All must match. Quotes make a phrase.
- `limit` (integer, optional, default 10, 1 to 50)
- `status` (string, optional): Only this status, for example accepted.
- `type` (string, optional): Only this node type, for example requirement or adr.

## `spec_api_search`

Find the operations of the API (requirements titled `GET /orders`) nearest in meaning to a question, best first, each with its call, requirement, score and the text it was indexed from. Needs the index built with `oxispec api index`; an error says so when it is missing or made with another embedder.

Read only.

Arguments:

- `query` (string, required): What the operation does, in words.
- `top` (integer, optional, default 5, 1 to 50)

## `spec_get`

One node: its text, its links, and the nodes that link to it.

Read only.

Arguments:

- `id` (string, required): A node ID such as REQ-AUTH-0001, or a scenario such as REQ-AUTH-0001.S1.
- `at` (string, optional): A git revision: a branch, tag, hash or date:YYYY-MM-DD. Default: the working tree.

## `spec_neighbors`

Nodes within a number of link hops of a node, in both directions, nearest first.

Read only.

Arguments:

- `id` (string, required): A node ID such as REQ-AUTH-0001, or a scenario such as REQ-AUTH-0001.S1.
- `depth` (integer, optional, default 1, 1 to 4)
- `kinds` (array of string, optional): Only follow these link kinds, for example decided-by.

## `spec_context`

The text of the given nodes and what is near them, nearest first, sized to a token budget. What does not fit is listed by ID.

Read only.

Arguments:

- `ids` (array of string, required)
- `at` (string, optional): A git revision: a branch, tag, hash or date:YYYY-MM-DD. Default: the working tree.
- `budget` (integer, optional, default 8000, 500 to 100000): Tokens, counted as one per four characters.
- `depth` (integer, optional, default 2, 0 to 4)

## `spec_check`

Run the structural checks. Returns every finding with its code, file and message.

Read only.

Arguments:

- `at` (string, optional): A git revision: a branch, tag, hash or date:YYYY-MM-DD. Default: the working tree.

## `spec_status`

A summary of the project: nodes by type and status, error and warning counts by code, change sets in flight, the share of requirements with claims and the latest release. Start here to see what needs attention.

Read only.

Arguments:

- `at` (string, optional): A git revision: a branch, tag, hash or date:YYYY-MM-DD. Default: the working tree.

## `spec_history`

How a node changed over time: commits, authors and dates.

Read only.

Arguments:

- `id` (string, required): A node ID such as REQ-AUTH-0001, or a scenario such as REQ-AUTH-0001.S1.
- `limit` (integer, optional, default 20, 1 to 100)

## `spec_diff`

How the specs differ between two revisions: nodes added, removed and changed, and findings that appeared or went away.

Read only.

Arguments:

- `from` (string, required): A git revision.
- `to` (string, optional): A git revision. Default: the working tree.

## `spec_trace`

Where a node is implemented and tested, from spec: and verifies: comments. Without an ID, every annotation.

Needs the `tests` module. Read only.

Arguments:

- `id` (string, optional): A node ID such as REQ-AUTH-0001, or a scenario such as REQ-AUTH-0001.S1.

## `spec_claims`

The vocabulary and the claims on each variable, with the share of requirements that have claims. Conflicts between claims come from spec_check.

Needs the `consistency` module. Read only.

Arguments:

- `variable` (string, optional): Only this variable, for example session.idle_timeout.

## `spec_entities`

The entities the specs are about (orders, users, sessions) with their typed attributes, and for each the nodes that depend on it: those that link to it with `about` or another link, those whose claims use its attributes, and the nodes that link to those. Use it before changing a requirement or an entity, to see what else the change touches.

Needs the `consistency` module. Read only.

Arguments:

- `depth` (integer, optional, default 2, 1 to 5): How many links away a dependency is followed.
- `id` (string, optional): Only this entity, for example ENT-0001. Without it, every entity with the number of dependents.

## `spec_architecture`

The architecture model: its elements, environments and rules, or with `view` and `id` one view as a Mermaid flowchart (context and container views are of a system, a component view of a container, a deployment view of an environment). Problems come from spec_check.

Needs the `architecture` module. Read only.

Arguments:

- `id` (string, optional): The system, container or environment the view is of.
- `view` (string, optional): Which view to draw. Needs `id`.

## `spec_debt`

The tech debt backlog, the most serious and the oldest first: each item's status, severity, owner, due date, age and what it arises from, flags for overdue and stale items, and the limits a release is held to. Problems come from spec_check.

Needs the `debt` module. Read only.

Arguments:

- `all` (boolean, optional): Include resolved and accepted items. Default: false.
- `severity` (string, optional): Only items with this severity.
- `status` (string, optional): Only items with this status.

## `spec_risks`

The risk register, the most serious level first and the oldest first: each risk's status, likelihood, impact, level, owner, review date, what mitigates it and what it arises from, and the live critical risks that hold a release back. Problems come from spec_check.

Needs the `risks` module. Read only.

Arguments:

- `all` (boolean, optional): Include risks that are not live (mitigated, accepted, closed). Default: false.
- `level` (string, optional): Only risks at this level.
- `status` (string, optional): Only risks with this status.

## `spec_machine`

State machines. Without an ID, every machine. With an ID, its transitions and each property with a counterexample path if it is violated, or a Mermaid diagram or one test per transition. Problems come from spec_check.

Needs the `statemachines` module. Read only.

Arguments:

- `id` (string, optional): A state machine ID such as SM-ORDER.
- `view` (string, optional): What to return for an ID. Default: summary.

## `spec_propose_change`

Write a draft change set that proposes a change to the specs, linked to the nodes it changes. It is not approved: a person reviews it. Edit the nodes themselves in the files.

Needs the `specs` module. Writes files. Not offered with `--read-only`, or by `oxispec serve`.

Arguments:

- `title` (string, required): A short title, one line.
- `changes` (array of string, optional): IDs of the nodes this change modifies.
- `why` (string, optional): Why the change is needed.
