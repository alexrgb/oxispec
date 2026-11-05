# Moving from another tool

OxiSpec reads OpenSpec, Spec Kit, Kiro, plain markdown, product requirements documents (`--from prd`) and OpenAPI or Swagger descriptions (`--from openapi`). Nothing is deleted or changed in the old folders.

## Specs only, in one step

```bash
oxispec init --preset standard
oxispec import specs --from openspec --dry-run   # what would be written
oxispec import specs --from openspec
oxispec check
```

Where it looks: `openspec/specs`, `specs` (Spec Kit), `.kiro/specs`; for `markdown`, `prd` and `openapi` pass `--dir` with a file or folder. `prd` reads the bullets under requirements, features and user-story sections; `openapi` writes a node for each tag, a block for each operation and a scenario for each response (see [profiles.md](../profiles.md#importing-specs-from-other-tools)). Each spec file becomes one requirement node; the report lists what was left out (design notes, tasks, plans and open changes).

The import does not invent content. Where a source has no scenario, it writes a placeholder, and where the project's style needs SHALL or MUST and the text has none, it adds a sentence that does. Each is a note in the report, and `oxispec check` shows what is left to fill in. A second run refuses to overwrite files.

## Specs, changes and design notes, with a say in each

Use a plan when there are open changes or design notes, or when you want to decide file by file:

```bash
oxispec import plan start --from openspec     # lists the items; writes .oxispec/import-plan.json
oxispec import plan show 2 --draft            # the file item 2 would write, with notes
oxispec import plan set 2 --kind adr          # requirement, change, adr or skip
oxispec import apply --dry-run                # everything that would be written
oxispec import apply                          # write it, remove the plan
```

By default a spec becomes a requirement, an open change (proposal, design, tasks, spec deltas) becomes a draft change set, and design notes next to a spec become a change set; archived changes are skipped. Nothing is written before `apply`.

An AI agent can do this with you: the `oxispec-import-specs` skill makes it propose, ask about each item and show the drafts, and apply only after your yes.

## Decision records

`oxispec import adr --from adr-tools|madr|log4brains` reads the records of those tools, with their statuses and "superseded by" lines, as ADRs. `oxispec adr convert` changes an ADR from one format to another. See [profiles](../profiles.md#importing-decisions-from-other-tools).
