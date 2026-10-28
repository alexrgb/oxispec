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

Other kinds are accepted with a warning, which catches typos.

Reverse links are computed, never stored. `oxispec show ADR-0001` lists the nodes that link to it.

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
| W001 | warning | Unknown node type |
| W002 | warning | Frontmatter is not valid YAML, so the file was skipped |
| W003 | warning | A file could not be read |
| W004 | warning | Unknown link kind |
| I001 | info | A link to another project is not checked. Shown with `--verbose`. |

`oxispec check` exits with 1 when there are errors, and also on warnings with `--strict`. Other failures, such as a missing project, exit with 2.
