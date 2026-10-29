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
| `type` | Required. `requirement`, `adr`, `decision`, `test-case`, `standard`, `api`, `state-machine`, `diagram`, `term`, `change`, `system`, `container`, `component`, `person`, `deployment-environment` or `release`. Other types give a warning, because later modules add types. |
| `status` | Optional, free text for now, for example `draft` or `accepted`. |
| `owner` | Optional, free text for now. |
| `approved_by`, `approved_on` | Set on [change sets](#change-sets) when they are approved. A name, and a date as `YYYY-MM-DD`. |
| `links` | Optional. Link kind to one or more targets. |

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

Other kinds are accepted with a warning, which catches typos.

Reverse links are computed, never stored. `oxispec show ADR-0001` lists the nodes that link to it.

## Change sets

A change set is a node of type `change`: a proposal to change the specs, with a lifecycle. Create one with `oxispec new change "Tighten timeouts"`, which writes `specs/changes/0001-tighten-timeouts.md` with sections for why, what changes and impact. Link it to what it changes with `oxispec link CHG-0001 changes REQ-AUTH-0001`.

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
- **The approval is recorded, not enforced.** Anyone who can edit the file can write a name into it. Until access control exists, what stops someone approving their own change is your repository's review rules. An agent should propose a change set and leave approval to a person.

## Releases

A release of the specs is a **manifest** node of type `release`, written by `oxispec release prepare` as `specs/releases/<version>.md`, and an annotated git tag `specs-v<version>` on the commit that holds it. Enable the `releases` module (`oxispec modules add releases`) to use it.

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

## Project config

`.oxispec/config.yaml`:

```yaml
format: 1
modules:
- core
- specs
topology: single
id_scheme: sequential
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
| W001 | warning | Unknown node type |
| W002 | warning | Frontmatter is not valid YAML, so the file was skipped |
| W003 | warning | A file could not be read |
| W004 | warning | Unknown link kind |
| I001 | info | A link to another project is not checked. Shown with `--verbose`. |

`oxispec check` exits with 1 when there are errors, and also on warnings with `--strict`. Other failures, such as a missing project, exit with 2.
