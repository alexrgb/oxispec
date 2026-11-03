# Format profiles

A **profile** says what a node of one type must contain: the sections it has, the front matter fields that must be set, and the statuses it may take. It also gives the template `oxispec new` writes. A project picks a profile for a type, and `oxispec check` holds the nodes of that type to it.

A type with no chosen profile is checked as before. Nothing changes until a project asks for it.

```bash
oxispec formats list                # the profiles, built in and the project's own
oxispec formats show lightweight    # what one requires, and its template
oxispec new adr "Use Postgres" --format lightweight
```

## Choosing a profile

In `.oxispec/config.yaml`, `formats` maps a node type to a profile name:

```yaml
formats:
  adr: lightweight
```

`oxispec new adr` then writes that profile's template. `--format NAME` picks another one for a single ADR. The profile has to be for the type, so `--format shall` on an ADR is refused.

## Your own profile

Define it under `profiles`:

```yaml
profiles:
  lightweight:
    type: adr
    description: A short ADR with an owner
    sections: [Context, Decision]      # required
    optional: [Consequences]           # in the template, not required
    fields: [owner]                    # front matter that must be set
    statuses: [proposed, accepted, superseded]
```

| Key | Meaning |
|---|---|
| `type` | The node type it is for. Required. |
| `sections` | Headings the node must have, at level 2 or 3 (`##` or `###`). Compared without regard to case. A heading inside a code block does not count. |
| `optional` | Sections the template includes but `check` does not require. |
| `fields` | Front matter fields that must be set and not empty: `status`, `owner`, `approved_by`, `approved_on` or `version`. |
| `statuses` | The values `status` may take. The first one is what `new` writes. Empty means any. |
| `description` | One line for `formats list`. |

The name is letters, digits, `-` and `_`, and cannot be the name of a built-in profile.

## Built-in profiles

| Name | Type | What it is |
|---|---|---|
| `default` | `adr` | Context, Decision and Consequences, with nothing required. It is what `new adr` has always written. |
| `shall` | `requirement` | A requirement with SHALL or MUST and scenarios, checked by E006 to E008. |

Choosing a built-in profile never makes an existing project fail. More profiles (ADR formats such as MADR and Nygard, and requirement styles such as EARS and Gherkin) are planned in [iterations.md](iterations.md), iterations 26 and 28.

## Checks

| Code | Meaning |
|---|---|
| E028 | A section the profile requires is missing. |
| E029 | A front matter field the profile requires is missing or empty. |
| E030 | The status is missing or not one the profile allows. |
| E031 | The config names a profile that does not exist, is for another type, or is not valid. The finding points at `.oxispec/config.yaml`. |

A node made by `new` from a profile with required fields starts with those fields empty, so `check` reports them until someone fills them in. That is on purpose: the template shows what has to be decided.

## Limits

- Profiles are read from the config of the working tree. `--at REVISION` and `history` read old revisions with no profiles, so they report no E028 to E030.
- `new` writes templates for ADRs. Requirement templates come with the requirement styles in iteration 28.
- A running `oxispec mcp` or `oxispec serve` notices a change to the config within a moment, like any other edit.
