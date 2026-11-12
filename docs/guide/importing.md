# Moving from another tool

OxiSpec reads OpenSpec, Spec Kit, Kiro, plain markdown, product requirements documents (`--from prd`) and OpenAPI or Swagger descriptions (`--from openapi`). Nothing is deleted or changed in the old folders.

## Specs only, in one step

```bash
oxispec init --preset standard
oxispec import specs --from openspec --dry-run   # what would be written
oxispec import specs --from openspec
oxispec check
```

Where it looks: `openspec/specs`, `specs` (Spec Kit), `.kiro/specs`; for `markdown`, `prd` and `openapi` pass `--dir` with a file or folder. `prd` reads the bullets under requirements, features and user-story sections; `openapi` writes a node for each tag, a block for each operation and a scenario for each response (see [profiles.md](../profiles.md#importing-specs-from-other-tools)). Each requirement in a spec file becomes a node of its own: a file of 12 requirements gives `REQ-AUTH-0001` to `REQ-AUTH-0012`, one file each, named after the requirement. That is what lets a comment in the code (`// spec: REQ-AUTH-0004`), a test (`// verifies: REQ-AUTH-0004.S2`), a claim or a change set point at one requirement and not at the whole file. The scenarios of a node start at `{#S1}`, the purpose of the spec is in the first node, and the id the source gave (`AUTH-4`) stays in the title. The report lists what was left out (design notes, tasks, plans and open changes).

The import does not invent content. Where a source has no scenario, it writes a placeholder, and where the project's style needs SHALL or MUST and the text has none, it adds a sentence that does. Each is a note in the report, and `oxispec check` shows what is left to fill in. A second run refuses to overwrite files.

## Specs, changes and design notes, with a say in each

Use a plan when there are open changes or design notes, or when you want to decide file by file:

```bash
oxispec import plan start --from openspec     # lists the items; writes .oxispec/import-plan.json
oxispec import plan show 2 --draft            # the file item 2 would write, with notes
oxispec import plan set 2 --kind adr          # requirement, change, adr or skip
oxispec import plan set --where kind=requirement --kind skip   # every item of a kind
oxispec import apply --dry-run                # everything that would be written
oxispec import apply                          # write it, remove the plan
```

`plan start --only change` plans the open changes and sets the rest to skip, which suits a project whose specs are imported already. `--status accepted`, on `import specs` and on `import apply`, gives the requirements the status of specs that are in force instead of the first status of the profile.

By default a spec becomes a requirement, an open change (proposal, design, tasks, spec deltas) becomes a draft change set, and design notes next to a spec become a change set; archived changes are skipped. Nothing is written before `apply`. If you ran `import specs` first, the plan lists those specs as skipped, so they are not written twice.

An AI agent can do this with you: the `oxispec-import-specs` skill makes it propose, ask about each item and show the drafts, and apply only after your yes.

After an import, `oxispec list --type requirement --domain carousels` lists the requirements of one domain (the part of the ID between `REQ-` and the number), and `oxispec show ID --text` prints a node's text.

## Decision records

`oxispec import adr --from adr-tools|madr|log4brains` reads the records of those tools, with their statuses and "superseded by" lines, as ADRs. A status may have words after it (`Accepted (built)`, `Accepted, amended by ADR-0003`): the status is taken from the first word, the rest is kept as a note, and an "amended by" becomes a link. The `adrs` module is switched on if it is off. `oxispec adr convert` changes an ADR from one format to another. See [profiles](../profiles.md#importing-decisions-from-other-tools).
