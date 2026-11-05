# Import specs from another tool

Bring the specs, changes and design notes of OpenSpec, Spec Kit, Kiro or plain markdown into the project, one decision at a time. The person decides what each file becomes. You propose, show drafts, and write only when they say so.

## Steps

1. Start a plan: `oxispec import plan start --from openspec --json` (`speckit`, `kiro` or `markdown --dir <path>` for the others). It writes `.oxispec/import-plan.json` and lists the items. Nothing else is written.
2. Read each item: its `kind` (requirement, change, adr or skip), its `role`, its `sources` and `why` it was proposed. Specs become requirement nodes. Open changes, and the design, tasks and plans next to a spec, become change sets. Archived changes are skipped.
3. For every item where another kind could be better, say so and ask. A design note that records one decision with its reasons may be an ADR. A file that only repeats other specs may be skipped. Do not decide this for the person.
4. Look at a draft before it is final: `oxispec import plan show N --draft`. Read the notes it prints. They list what is still to fill in: a requirement with no scenario, a change set with no "why", a decision that is not written yet.
5. Change an item: `oxispec import plan set N --kind adr` or `--title "Better name"`. Look at its draft again.
6. Show the whole result: `oxispec import apply --dry-run`. Give the person the list of files, the notes and what was skipped.
7. Only when they agree, run `oxispec import apply`. It writes the nodes and removes the plan. Then run `oxispec check` and report what it finds.

## Rules

- Never run `oxispec import apply` without the person's yes after a dry run.
- Imported change sets are `draft`. Do not approve them; a person does that.
- Do not edit the imported text to make `check` pass before the person has seen it. Report the findings, then fix with them.
- Do not invent what is missing: a decision, a reason, a scenario. Leave the placeholder and say it is still to be written.
- `oxispec import specs --from <tool>` imports only the specs, in one step, with no plan. Use the plan when there are changes or design notes.
