# Concepts

## Nodes

Everything is a **node**: a markdown file whose front matter has an `id` and a `type`. A requirement, a decision record (ADR), a test case, a change set and a tech debt item are all nodes. A markdown file without those two keys, such as a README, is not a node and is left alone, so documents can sit next to the specs.

```markdown
---
id: REQ-AUTH-0012
type: requirement
status: accepted
links:
  decided-by: [ADR-0007]
---
```

Files live where you like, but `oxispec new` puts them in `specs/`: `specs/requirements/<domain>/`, `specs/adr/`, `specs/changes/`, `specs/debt/`, `specs/prd/`.

## IDs

An ID never changes and is never reused. It is letters and digits in hyphen-separated segments: `REQ-AUTH-0012`, `ADR-0007`, `CHG-0003`. The number is sequential by default; with `oxispec init --id-scheme random` it is a short random suffix (`REQ-AUTH-7K3F`), which stops several branches from taking the same number.

A scenario is addressed by its anchor: `REQ-AUTH-0012.S1`.

## Links

A link has a kind and a target, and is written in the front matter of the node it starts from: `decided-by`, `satisfies`, `verifies`, `implements`, `refines`, `supersedes`, `changes`, `depends-on` and a few more (the list is in [format](../format.md#links)). `oxispec link FROM KIND TO` adds one and refuses a target that does not exist. The reverse direction is computed, never stored, so `oxispec show ID` always lists what points at a node.

## Modules and presets

The project's `config.yaml` lists the modules that are on. A module adds node types, commands and checks. Presets are bundles: `minimal`, `specs-tests`, `docs`, `standard`, `full`. `oxispec modules` lists them with what each is for, and `oxispec modules add debt` turns one on. A command of a module that is off tells you which module it needs.

## Checks

`oxispec check` reads every node and reports:

- **errors** (exit code 1): duplicate IDs, links to nodes or scenarios that do not exist, a requirement without a scenario or a SHALL or MUST, an unknown status, a missing approver;
- **warnings**: unknown types or link kinds, unreachable states, debt with no owner.

The table of codes is in [format](../format.md#check-codes). Checks do not change files.

## History

The specs are plain files in git, so history is git's. `oxispec history ID` shows how a node changed, `oxispec diff v1` compares two revisions, `oxispec tag NAME` marks a revision of the specs, and `oxispec rollback` puts nodes back after checking that nothing breaks. See [history](../history.md).

## Everything has a JSON form

Most commands accept `--json` for scripts and agents, with a `schema_version` field. The same data is served to AI clients by `oxispec mcp` (one process) or `oxispec serve` (many clients over HTTP). See [MCP](../mcp.md).
