# Agent skills

Skills tell an AI coding agent how to work with the specs: how to look things up, propose a change, tie code to a requirement and verify the result. They follow the open [Agent Skills](https://agentskills.io) format: a folder per skill with a `SKILL.md`, a name and a one-line description. `oxispec` writes them into the folder each client reads. They need the `agents` module (`oxispec modules add agents`, included in the `standard` preset).

## What you get

| Skill | Needs module | For |
|---|---|---|
| `oxispec-explore` | core | Finding and reading nodes, history and links before writing |
| `oxispec-propose` | specs | A change set with new or edited requirements, left for a person to approve |
| `oxispec-extract-claims` | consistency | Writing claims and the vocabulary for a requirement, and what to do with a conflict |
| `oxispec-model-architecture` | architecture | Modelling systems, containers, components and environments, rules, and drawing views |
| `oxispec-model-state-machines` | statemachines | Writing a state machine and its properties, and what to do with a violated one |
| `oxispec-apply` | specs | Implementing an approved change, with `spec:` and `verifies:` comments |
| `oxispec-verify` | specs | `check`, `coverage`, test results and suspect links, in order |
| `oxispec-sync-code` | tests | Deciding which side is right for a suspect link, then confirming |
| `oxispec-write-adr` | adrs | A decision record with the options that were rejected |
| `oxispec-write-prd` | specs | A PRD as nodes: interviews the person, shows a draft, then writes the `prd` node and the requirements that refine it |
| `oxispec-write-api` | specs | An API as requirements titled as calls, the OpenAPI skeleton made from them, and the comparison with a real file |
| `oxispec-import-specs` | specs | Moves specs, open changes and design notes from another tool in, with the person deciding what each file becomes |
| `oxispec-manage-debt` | debt | Recording a shortcut as a debt item, keeping the backlog honest and paying items off |
| `oxispec-review-change` | core | Reviewing a branch with `ci`, `diff` and `suspect` |
| `oxispec-rollback` | core | Restoring nodes with `--dry-run` first |
| `oxispec-release` | releases | Preparing, checking and publishing a spec release |

Nothing is written for a module that is off, so an agent only sees the skills that apply. The skills never approve, merge or publish on their own. They tell the agent to leave that to a person.

The skill for decision logs comes with the module that supports it.

## Where the files go

| Client | Folder |
|---|---|
| `claude` (Claude Code) | `.claude/skills/` |
| `cursor` (Cursor) | `.cursor/skills/` |
| `copilot` (GitHub Copilot) | `.github/skills/` |
| `gemini` (Gemini CLI) | `.gemini/skills/` |
| `agents` | `.agents/skills/` |

Those are the project folders in each client's documentation. `agents` is a neutral folder that Cursor, Copilot and Gemini CLI read as well, so one copy serves all of them. It is not in the default, because with the four native folders a client that also reads another client's folder sees a skill twice. If your team uses several clients, `--agents agents` gives you one copy.

```bash
oxispec init --preset standard                       # the four native folders
oxispec init --preset standard --agents claude       # one client
oxispec init --preset standard --agents agents       # the neutral folder
oxispec init --preset standard --agents none         # no files now
oxispec agents list                                  # clients, skills, and what this project gets
oxispec agents update                                # write or refresh
oxispec agents update --client cursor --dry-run      # one client, show only
oxispec agents check                                 # exit 1 on drift, for CI
```

The chosen clients are stored in `.oxispec/config.yaml` as `agents:`. Commit the generated folders, so everyone on the team and in CI gets the same skills.

## Hand edits are kept

Each file has a hash of its text in the frontmatter (`metadata.content-hash`).

- **Not edited, and oxispec now writes other text** (a new version, or a new project note): `update` replaces it.
- **Edited by hand**, or a `SKILL.md` that oxispec did not write: `update` skips it, says so, and exits with 1. `update --force` replaces it.
- `check` reports a missing file, an out-of-date file and an edited file, and exits with 1 on any of them. Put it in CI to catch a skill folder that nobody regenerated after upgrading.

Line endings do not count as an edit, so a checkout with CRLF is still current.

## Project notes

To add text of your own to every skill, put it in `.oxispec/agents/`:

```text
.oxispec/agents/_all.md     every client
.oxispec/agents/claude.md   appended after _all.md for that client (also cursor.md, copilot.md, gemini.md, agents.md)
```

The text goes into each skill under `## Project notes`. Run `oxispec agents update` after changing it. Prefer notes that are specific to your project, such as which domains exist or which task runner to use. The skills already cover how to use `oxispec`.

## Other tools

`oxispec agents export` writes the skills in the layout another tool reads, for the project's enabled modules and project notes:

```bash
oxispec agents export --to ruler --out .          # .ruler/skills/<name>/SKILL.md, then run `ruler apply`
oxispec agents export --to skills-repo --out .    # skills/<name>/SKILL.md, for `npx skills add <owner>/<repo>`
oxispec agents export --to skills-repo --all      # every skill, no project notes, no project needed
```

Export overwrites what is there. `--all` is how the `skills/` folder in the OxiSpec repository is made, so that `npx skills add <owner>/oxispec` works. A test fails when that folder differs from what the generator writes, and another test runs every `oxispec` command and flag named in the skills against the real CLI, so the skills cannot drift from the commands.

Not done yet: a layout for Rulesync (its skills folder is not in its README, so it is not guessed here), and writing into `.ruler/` automatically when the folder exists. Slash commands with tool permissions are not written. The MCP server exists now ([mcp.md](mcp.md)), but its entry in each client's config is not written for you yet.
