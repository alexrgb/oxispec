# Spec format

How OxiSpec reads your files. This describes what `oxispec check` accepts today. The full design is in [design.md](design.md), and how formats are versioned is in [versioning.md](versioning.md).

## Nodes

A **node** is a markdown file that starts with YAML frontmatter containing an `id` and a `type`. Files without frontmatter, or with frontmatter that has neither key, are ignored, so READMEs and site pages can live next to specs. A file with only one of the two keys is an error.

```markdown
---
id: REQ-AUTH-0012
type: requirement
status: accepted
owner: team-identity
links:
  decided-by:
    - ADR-0007
  refines:
    - REQ-AUTH-0004.S1
---
### Requirement: Admin session timeout
The system MUST expire admin sessions after 15 minutes of inactivity.

#### Scenario: Idle admin {#S1}
- **WHEN** an admin is inactive for 15 minutes
- **THEN** the session is invalidated
```

| Key | Meaning |
|---|---|
| `id` | Required. Never changes and is never reused. |
| `type` | Required. `requirement`, `adr`, `decision`, `test-case`, `standard`, `api`, `state-machine`, `diagram`, `term`, `change`, `system`, `container`, `component`, `person`, `deployment-environment`, `release`, `debt`, `prd`, `risk`, `test-plan` or `entity`. Other types give a warning, because later modules add types. |
| `status` | Optional, free text for now, for example `draft` or `accepted`. |
| `owner` | Optional, free text for now. |
| `approved_by`, `approved_on` | Set on [change sets](#change-sets) when they are approved. A name, and a date as `YYYY-MM-DD`. |
| `links` | Optional. Link kind to one or more targets. |
| `aliases` | Optional. Other names the node answers to, such as the ID it had in another tool (`aliases: [CAR-1]`). A link, a `spec:` or `verifies:` marker in code, `show`, `trace` and `search` may use one in place of the ID. An alias must be a valid ID that no node has and no other node lists (E042). |

Unknown keys are ignored.

## IDs

An ID is letters and digits in segments separated by hyphens, starting with a letter, with at least two segments: `REQ-AUTH-0012`, `ADR-0007`, `C4-orders-api`.

| Form | Example | Meaning |
|---|---|---|
| Node | `REQ-AUTH-0012` | A node in this project |
| Scenario | `REQ-AUTH-0012.S1` | A scenario of a requirement, by its anchor |
| External | `product/checkout:REQ-CHK-0004` | A node in another project. Accepted, but not checked yet. |

**ID schemes.** `oxispec init --id-scheme` sets how `oxispec new` numbers things. It is stored as `id_scheme` in the config, and `oxispec new ... --id-scheme` overrides it for one node.

| Scheme | Example | Use when |
|---|---|---|
| `sequential` (default) | `REQ-AUTH-0001` | One branch adds specs at a time |
| `random` | `REQ-AUTH-7K3F` | Several branches add specs in parallel. A four character suffix from 32 readable characters, close to a million values, checked against the specs on disk. A random suffix always contains a letter, so it is never mistaken for a sequential number. |

## Requirements and scenarios

A requirement node holds one or more `### Requirement:` blocks. Each block needs:

- a description that contains **SHALL** or **MUST** in capitals
- at least one `#### Scenario:` heading (four hashes)

Headings inside code fences are ignored.

**Scenario anchors.** End a scenario heading with `{#S1}` to make it linkable. An anchor is letters and digits, and is unique within the file. `oxispec new requirement` writes `{#S1}` for you.

## Links

Links connect nodes. Add them by hand or with the command:

```bash
oxispec link REQ-AUTH-0002 refines REQ-AUTH-0001.S1
oxispec link REQ-AUTH-0001 decided-by ADR-0001
```

`oxispec link` checks that the target exists (and that the scenario exists, for a scenario target), refuses a link to the node itself, and does nothing if the link is already there. It rewrites only the `links:` block, one target per line so parallel edits rarely conflict, and leaves the rest of the file as it was.

| Kind | Meaning |
|---|---|
| `satisfies` | This node meets that requirement |
| `verifies` | This test case checks that scenario |
| `implements` | This code or component implements that requirement |
| `decided-by` | This node follows that decision |
| `supersedes` | This node replaces that one. Cycles are errors. |
| `refines` | This node adds detail to that one |
| `governed-by` | This node follows that standard |
| `describes` | This diagram or document describes that node |
| `overrides` | This node deliberately overrides that one |
| `conflicts-with` | A declared conflict |
| `depends-on` | This node needs that one |
| `changes` | This change set modifies that node |
| `includes` | This release includes that change set |
| `arises-from` | This debt item is a compromise of that requirement, decision or component |
| `amends` | This decision changes part of that one, which stays in force |
| `clarifies` | This decision explains that one without changing it |

Other kinds are accepted with a warning, which catches typos.

Reverse links are computed, never stored. `oxispec show ADR-0001` lists the nodes that link to it.

## Change sets

A change set is a node of type `change`: a proposal to change the specs, with a lifecycle. Create one with `oxispec new change "Tighten timeouts"`, which writes `oxispec/changes/0001-tighten-timeouts.md` with sections for why, what changes and impact. Link it to what it changes with `oxispec link CHG-0001 changes REQ-AUTH-0001`.

```text
draft -> in-review -> approved -> merged -> reverted
              ^            |
              +------------+   changes after approval go back to review
```

```bash
oxispec change status CHG-0001 in-review
oxispec change status CHG-0001 approved --by "Ann Lee"   # default: git user.name
oxispec change status CHG-0001 merged
oxispec change list --status approved                    # also --json
```

- Steps cannot be skipped, and `--force` allows it.
- Approving records `approved_by` and `approved_on` in the file, so the approval is in git history with the commit that made it. Merging keeps them. Moving back to draft or review withdraws them.
- `oxispec check` reports a change set with a missing or unknown status (E012) and an approved or merged one with no approver (E013).
- **Tasks.** The checkboxes under the `## Tasks` heading of a change set are its tasks (`oxispec new change` writes the heading): `- [ ] 1.2 Guard the update (REQ-POST-0003.S2)`. A leading number such as `1.2` is kept apart from the text, and an ID written in the text (a requirement, or a scenario with `.S2`) links the task to it. `show CHG-0001` prints `tasks: 7/9 done` with each task, `change list` adds `[7/9 tasks]` (and `--json` has `tasks.done` and `tasks.total`), and `show REQ-POST-0003` lists the tasks that name it. `check` warns about a task that names an ID that does not exist (W027; a word such as `TC-POST-040` is left alone unless nodes of that kind exist) and about a merged change set with tasks still open (W028). `oxispec change task done CHG-0001 1.1 #3` ticks tasks by number or by place in the list, `change task undo` unticks them, and `--all` takes every task; only the checkbox on the line changes, and a number used by two tasks is refused (use `#N`). Tasks come from OpenSpec's `tasks.md` when a change is imported.
- **The approval is recorded, not enforced.** Anyone who can edit the file can write a name into it. Until access control exists, what stops someone approving their own change is your repository's review rules. An agent should propose a change set and leave approval to a person.

## Claims and the vocabulary

A `claims` block in any node, and `vocabulary` blocks in nodes of type `vocabulary`, feed the consistency checks (E017 to E019). The syntax and the checks are in [claims.md](claims.md).

## State machines

A `statemachine` block in any node feeds the behavior checks (E020 to E024, W007, W008). The syntax and the checks are in [machines.md](machines.md).

## Architecture

An `architecture` block in an element or environment node feeds the model checks (E025 to E027, W009, W010) and the views. See [architecture.md](architecture.md).

## Releases

A release of the specs is a **manifest** node of type `release`, written by `oxispec release prepare` as `oxispec/releases/<version>.md`, and an annotated git tag `specs-v<version>` on the commit that holds it. Enable the `releases` module (`oxispec modules add releases`) to use it.

```yaml
---
id: REL-0001
type: release
status: candidate
version: "0.2.0"
links:
  includes: [CHG-0001, CHG-0003]
---
# REL-0001: Release 0.2.0

Changes since 0.1.0.

## Breaking
## Added
## Changed
## Change sets
```

```bash
oxispec release prepare              # the suggested version, notes, and the manifest file
oxispec release prepare 0.2.0        # choose the version (--force rewrites the notes, --dry-run writes nothing)
oxispec release check 0.2.0          # the gates, exit code 1 when not ready
oxispec release publish 0.2.0        # runs the gates on HEAD, then tags it specs-v0.2.0
oxispec release list                 # candidates and published releases
```

- **The release is the tag.** A manifest is a *candidate* until its tag exists, so publishing never edits a file. Commit the manifest first (a pull request is a good place to review the notes), then publish. Push the tag yourself.
- **The base** is the newest published, final release below the new version (tags `specs-v<semver>`, never a pre-release). With none, this is the first release and the suggestion is `0.1.0`.
- **Notes** come from the graph diff between the base and now: removed requirements or scenarios are *Breaking*, new nodes, requirements or scenarios are *Added*, any other change is *Changed*. Change sets and release manifests are not counted as changes. The change sets listed are the ones added or edited since the base.
- **The suggested version** follows [versioning.md](versioning.md#versions-of-the-specs-you-ship): breaking is major, added is minor, anything else is a patch, and before 1.0 a breaking change bumps the minor version. You confirm or choose another version.
- **Gates**, run by `check` and again by `publish`, which runs them on the committed state so the tag is on exactly what was checked:

| Gate | Blocks | Passes when |
|---|---|---|
| `not-released` | yes | the version has no tag yet |
| `version-increases` | yes | it is newer than the base |
| `has-changes` | yes | something changed since the base (not for the first release) |
| `no-errors` | yes | `oxispec check` reports no errors |
| `change-sets-approved` | yes | every included change set is approved or merged |
| `manifest-current` | yes | the manifest lists the included change sets (otherwise `prepare --force`) |
| `version-matches-changes` | no, warns | the version is at least the suggested one |
| `no-suspect-links`, `tests-passing`, `results-current` | yes, yes, no | only for a project that uses trace annotations: see [trace.md](trace.md#release-gates) |

Not built yet, from the design: pinned code commits, test results and coverage thresholds as gates (they need the scanners), `deprecate`, `release diff`, supported-until dates, and release lines.

## Where the documents are

`oxispec init` creates the documents of a project in `oxispec/`: `oxispec/requirements/<domain>/`, `oxispec/adr/`, `oxispec/changes/`, `oxispec/releases/` and the folders of the other modules; the architecture model is in `oxispec/architecture/`. `oxispec init --docs-dir docs/spec` puts them in another folder, which is stored as `docs_dir` in the config. It is a relative path of plain folder names, with none that starts with a dot; `init` and every command that reads the config refuse anything else. Files that are already there are found wherever they are, because nodes are found by their front matter, so the folder can be changed at any time with `oxispec move-docs DIR`.

**Moving the documents.** `oxispec move-docs docs/spec` moves the folders of the documents (`requirements`, `adr`, `changes`, `releases` and the others that exist, and the architecture model) to the new folder and sets `docs_dir`. IDs and links name no paths, so nothing inside a file changes, and `check` finds the same nodes. `--dry-run` shows the moves and writes nothing. Either way the command first reports the new folder: whether it exists and what is in it, then each folder of the documents with where it goes and whether it `can move` or has a `CLASH` (the new folder already has a folder or file of that name with something in it). The move is all or nothing, so that `docs_dir` can name one folder: if anything clashes, nothing moves and the command exits with an error that names the folders to rename or move away in the new folder; the names of OxiSpec's own folders do not change. It refuses, before anything moves, a folder that is the same as the old one, inside it or around it, a folder that is not inside the project, and a folder that already has a folder of the same name with something in it. A failure part way undoes the moves. The command lists the files, such as a README, that name the old folder; it does not change them. It moves the files on disk and leaves git alone, so `git add -A` then stages them as renames. Moving back to `specs` sets `docs_dir: specs`.

A config with no `docs_dir`, as projects had before the folder could be chosen, means `specs` (and `architecture/model` for the architecture model), so nothing has to move; the key is written, as `docs_dir: specs`, the next time the config is saved. This repository's own specs are one such project.

## Project config

`.oxispec/config.yaml`:

```yaml
format: 1
modules:
- core
- specs
topology: single
id_scheme: sequential
docs_dir: oxispec          # where documents are written; a config without it means `specs`
agents: [claude, cursor]   # only with the agents module; see agents.md
trace:
  exclude: ["vendor/**"]   # optional
  languages: [rust, python]   # optional: scan only these, see trace.md
  test_ids: ['TC-[A-Z]+-[0-9]+']   # optional: IDs in test titles count as `verifies`, see trace.md
ids:                        # optional: IDs of your own shape need no node, see trace.md
  TC: 'TC-[A-Z]+-[0-9]+'
external:                   # optional: IDs of another system, with an optional link, see trace.md
  KAN: { pattern: 'KAN-[0-9]+', url: 'https://example.atlassian.net/browse/{id}' }
sources:                    # optional: rows of tables are nodes, see trace.md
  - files: 'docs/*/test-cases.md'
    table: { id: 1, id_pattern: 'TC-[A-Z]+-[0-9]+', type: test-case, title: 2, links: { verifies: 3 } }
  - files: 'docs/*/spec.md'
    heading: { id_pattern: 'US-[A-Z]+-[0-9]+', type: user-story }
domains:                    # optional: the domain an import gives, see profiles.md
  analytics-and-reports: REPORTS   # a spec by name
  CAR: SLIDES                      # or the prefix of a source ID
api_index:                  # optional: embed with a service, see the API guide
  embedder: { url: "http://localhost:11434/v1/embeddings", model: nomic-embed-text }
```

`format` is the project format version. A project without it is read as format 1. A project with a higher number than this version knows stops with a message to upgrade oxispec.

## Local index

`oxispec` keeps a cache of parsed nodes in `.oxispec/cache/` so that a second run does not parse files that did not change. It is git-ignored, and safe to delete at any time.

- A file is reused without being read when its size and modification time match the cache. Otherwise it is read and hashed (BLAKE3), and reused when the hash matches. A file modified in the 2 seconds before the cache was written is always hashed, because it may have changed again within the same timestamp tick.
- A cache from another version of `oxispec`, or a damaged one, is ignored and replaced. A cache that cannot be written is skipped. The answers are the same either way, only slower.
- `oxispec index` builds or refreshes the cache and prints how many files it parsed and reused. `oxispec index --rebuild` starts again from nothing.
- `--no-cache` on any command neither reads nor writes the cache.
- If you restore a file's modification time after changing its content, the size-and-time shortcut cannot see the change. Run `oxispec index --rebuild`.

Measured on 20,000 requirement files (release build, macOS, Apple silicon): a full parse takes about 3.5 s, a run with a warm cache about 0.25 s, and one edited file is re-parsed alone. Cold start with a cache write takes about 4.6 s. The cache file is about 8 MB. These are one machine's numbers, not a promise.

## Checking only what changed

```bash
oxispec check --changed              # compared with HEAD
oxispec check --changed --base main  # compared with another ref
```

`--changed` reports findings for the spec files that differ from the base (staged, unstaged and new files), and for the nodes that link to the changed nodes. An old error in a file nobody touched, and not linked to anything that changed, is not reported, so a pre-commit hook does not fail on someone else's work. It needs a git repository.

A node you delete or rename is still found: its old ID is read from git, so the nodes that linked to it are checked and their dangling links are reported. In a repository with no commits yet, every spec file counts as changed.

The JSON output adds `checked_nodes` and a `scope` object (`base` and `changed_files`, or `null` for a full check).

## Machine output

`oxispec check --json` and `oxispec show --json` include `"schema_version": 1`. Adding fields does not change it. Removing, renaming or retyping a field does.

Each finding has `code`, `severity` (`error`, `warning` or `info`), `message`, and, when known, `path`, `line` and `id`.

## Check codes

Codes are stable. A code is never reused.

| Code | Severity | Meaning |
|---|---|---|
| E001 | error | Two nodes have the same ID |
| E002 | error | A link points to a node that does not exist |
| E003 | error | Frontmatter has `id` or `type`, but not both |
| E004 | error | `supersedes` links form a cycle |
| E005 | error | An ID is not valid (a node ID, or a link target) |
| E006 | error | A requirement has no scenario |
| E007 | error | A requirement has no SHALL or MUST |
| E008 | error | A requirement node has no `### Requirement:` block |
| E009 | error | A link points to a scenario that does not exist |
| E010 | error | A scenario anchor is used twice in one file |
| E011 | error | A scenario anchor is not letters and digits |
| E012 | error | A change set has no status, or one that is not in the lifecycle |
| E013 | error | A change set is approved or merged but has no `approved_by` |
| E014 | error | A `spec:` or `verifies:` comment in code refers to a node that does not exist (reported by `oxispec coverage`) |
| E015 | error | A `spec:` or `verifies:` comment in code refers to a scenario that does not exist (reported by `oxispec coverage`) |
| E016 | error | A link between a spec and code is suspect: either one changed since it was confirmed (reported by `oxispec suspect` and `oxispec ci`) |
| E017 | error | A line of a `claims` block cannot be read: bad syntax, an unknown variable or value, or a unit that does not fit the variable (see [claims.md](claims.md)) |
| E018 | error | A `vocabulary` block cannot be read, or declares a variable twice |
| E019 | error | Claims conflict: in some context where their scopes hold, no value satisfies them all |
| E020 | error | A `statemachine` block cannot be read (see [machines.md](machines.md)) |
| E021 | error | A state machine names a state that it does not declare |
| E022 | error | A state machine has two transitions from one state on one event that can both apply |
| E023 | error | A state machine breaks a property, and the message gives the path |
| E024 | error | A state machine links to a node that does not exist |
| E025 | error | An `architecture` block cannot be read (see [architecture.md](architecture.md)) |
| E026 | error | An architecture element, environment or rule refers to a parent, element or host that is missing or of the wrong kind |
| E027 | error | An architecture rule is broken, and the message gives the path |
| E028 | error | A section that the node's format profile requires is missing (see [profiles.md](profiles.md)) |
| E029 | error | A front matter field that the node's format profile requires is missing |
| E030 | error | A status is missing or not one that the node's format profile allows |
| E031 | error | The config names a format profile that does not exist or is not valid |
| E032 | error | A debt item has a status, severity or date that is not valid (see [debt.md](debt.md)) |
| E033 | error | A requirement is not worded the way its style asks, EARS or user story (see [profiles.md](profiles.md#requirement-styles)) |
| E034 | error | A scenario does not have the steps its style asks, a WHEN and a THEN for Gherkin |
| E035 | error | A node lacks a link the compliance rules ask for (see [compliance.md](compliance.md)) |
| E036 | error | A node lacks the sign-off the compliance rules ask for |
| E037 | error | The `compliance:` settings in the config are not valid |
| E038 | error | A risk has a status, likelihood, impact or date that is not valid (see [risks.md](risks.md)) |
| E039 | error | A risk is accepted but no person is named in `approved_by` |
| E040 | error | A test plan has a status or date that is not valid (see [test-plans.md](test-plans.md)) |
| E041 | error | An entity block cannot be read, has no name, or shares its name with another entity (see [entities.md](entities.md)) |
| E042 | error | An alias is not a valid ID, is the ID of another node, or is an alias of two nodes |
| W001 | warning | Unknown node type |
| W002 | warning | Frontmatter is not valid YAML, so the file was skipped. The finding is at the line of the error and says when a value needs quotes (a title with `: ` in it) |
| W003 | warning | A file could not be read |
| W004 | warning | Unknown link kind |
| W005 | warning | A `should` claim cannot hold in a context where other claims apply |
| W006 | warning | A claim never applies, because its scope can never hold |
| W007 | warning | A state cannot be reached from the initial state |
| W008 | warning | A state has no way out and is not final |
| W009 | warning | A container has no owner or no technology |
| W010 | warning | A container runs in no deployment environment |
| W011 | warning | A decision is marked superseded, but no decision in force links `supersedes` to it (see [profiles.md](profiles.md)) |
| W012 | warning | A decision is superseded by another, but its status is still proposed or accepted |
| W013 | warning | A debt item that is still to be paid has no owner |
| W014 | warning | A debt item is past its due date |
| W015 | warning | A debt item has not been reviewed for longer than `debt.stale_days` |
| W016 | warning | A debt item has no `arises-from` link |
| W017 | warning | A live risk has no owner |
| W018 | warning | A live risk is past its review date |
| W019 | warning | A risk is being mitigated but has no `mitigated-by` link |
| W020 | warning | A test plan covers nothing |
| W021 | warning | A requirement is accepted and no approved test plan covers it |
| W022 | warning | A live risk at a high level is covered by no approved test plan |
| W023 | warning | An operation states no 401 or 403 response, though other operations do (see [guide](guide/writing-an-api.md#security)) |
| W024 | warning | A node is `about` something that is not an entity |
| W025 | warning | An attribute of an entity has no field in the code that represents it (see [entities.md](entities.md#say-which-code-represents-it)) |
| W026 | warning | A node is about an entity that changed since the node was confirmed (see [entities.md](entities.md#look-again-when-it-changes)) |
| W027 | warning | A task of a change set names a node or scenario that does not exist (see [change sets](#change-sets)) |
| W028 | warning | A change set is merged but some of its tasks are still open |
| W029 | warning | The requirements of one domain are in more than one folder |
| W030 | warning | A row of a table that `sources:` reads has no ID, so it is not a node |
| I001 | info | A link to another project is not checked. Shown with `--verbose`. A link `product:ID` to a project of the same workspace is checked, see [getting started](guide/getting-started.md#a-platform-and-the-products-that-depend-on-it). |
| I002 | info | A claim says nothing that another claim does not already say. Shown with `--verbose`. |

`oxispec check` exits with 1 when there are errors, and also on warnings with `--strict`. Other failures, such as a missing project, exit with 2.
