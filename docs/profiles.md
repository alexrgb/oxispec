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
| `fields` | Front matter fields that must be set and not empty: `status`, `owner`, `approved_by`, `approved_on`, `version`, `date` or `decision-makers`. |
| `optional_fields` | Fields the template includes but `check` does not require. `date` is written as today's date. |
| `statuses` | The values `status` may take. The first one is what `new` writes. Empty means any. |
| `description` | One line for `formats list`. |

The name is letters, digits, `-` and `_`, and cannot be the name of a built-in profile.

## Built-in profiles

| Name | Type | What it is |
|---|---|---|
| `default` | `adr` | Context, Decision and Consequences, with nothing required. It is what `new adr` has always written. |
| `shall` | `requirement` | A requirement with SHALL or MUST and scenarios, checked by E006 to E008. |
| `nygard` | `adr` | Michael Nygard's format: Context, Decision, Consequences (all required), and a status. |
| `madr` | `adr` | [MADR 4.0](https://adr.github.io/madr/): Context and Problem Statement, Considered Options and Decision Outcome are required. Decision Drivers, Consequences, Confirmation, Pros and Cons of the Options and More Information are in the template and optional. `date` and `decision-makers` are in the template. |
| `y-statement` | `adr` | A decision in one sentence (context, concern, choice, neglected options, quality, downside, rationale), in a `Statement` section. |

Choosing `default` or `shall` never makes an existing project fail. Choosing one of the others holds every ADR to it, so a project that has ADRs in another shape will see E028 until they are converted (planned in iteration 26b). More profiles (requirement styles such as EARS and Gherkin) are planned in [iterations.md](iterations.md), iteration 28.

## ADR formats side by side

The three ADR formats keep the same facts in the same places, so a record can move from one to another.

- **Status, date and deciders are front matter**, not text in the body: `status`, `date`, `decision-makers`. Nygard's Status section and MADR's `status:` line are the same thing here.
- **Who replaced whom is a link.** `oxispec link ADR-0002 supersedes ADR-0001` says it. "Superseded by ADR-0002" is worked out from that link whenever it is shown, and never stored.
- **The status words** are `proposed`, `accepted`, `deprecated` and `superseded`, and `rejected` for MADR and the Y-statement.

| Nygard | MADR | Y-statement |
|---|---|---|
| Context | Context and Problem Statement | "In the context of ..., facing ..." |
| Decision | Decision Outcome ("Chosen option") | "we decided for ..." |
| Consequences | Consequences, under Decision Outcome | "to achieve ..., accepting ..." |
| | Considered Options, Pros and Cons | "and neglected ..." |
| | Decision Drivers | "because ..." |

## Superseded decisions

When the chosen profile of a type has the status `superseded`, `check` holds its nodes to the link:

| Code | Meaning |
|---|---|
| W011 | A decision is marked `superseded`, but no decision in force links `supersedes` to it. |
| W012 | A decision is `proposed` or `accepted`, but a decision in force links `supersedes` to it. |

"In force" means the successor is not `proposed` or `rejected`: a successor that has not been accepted yet has not replaced anything. A decision that is `deprecated` is left alone. These are warnings, and `--strict` makes them fail. A project whose profile has no `superseded` status sees neither.

## Requirement styles

A requirement profile can word a requirement in a style. The graph does not change with it: a requirement is still a `### Requirement:` block with scenarios, so links and coverage work the same. The style adds a check on the wording. Choose it with `formats:` in the config, as for any profile, and `oxispec new requirement auth "Login" --format ears` writes a requirement in it.

| Profile | What `check` asks for |
|---|---|
| `shall` | The default and today's form: SHALL or MUST in the description (E007), and at least one scenario (E006). |
| `ears` | The description is one of the six EARS patterns (E033). `shall` is read in any case, so E007, which wants capitals, is not raised. |
| `gherkin` | Every scenario has a WHEN and a THEN step, and may have GIVEN, AND and BUT (E034). E006 and E007 still apply. |
| `user-story` | The description is `As a <role>, I want <goal>, so that <benefit>` (E033). It needs no SHALL, so E007 is not raised. The scenarios are its acceptance criteria, and E006 still asks for one. |

The EARS patterns, with `shall` or `must`, in any case:

| Pattern | Form |
|---|---|
| Ubiquitous | The system shall log every login. |
| State-driven | While the door is open, the alarm shall stay silent. |
| Event-driven | When the user presses stop, the player shall pause. |
| Optional feature | Where a camera is fitted, the app shall offer a scan. |
| Unwanted behaviour | If the card is declined, then the shop shall ask for another. (`then` may be left out.) |
| Complex | While in standby, when the lid opens, the laptop shall wake. |

A clause that starts with `while`, `when`, `if` or `where` ends with a comma, then comes the system, then `shall`. The message of E033 says which part is missing.

A step is a line that starts with GIVEN, WHEN, THEN, AND or BUT, with or without a list marker or bold: `- **WHEN** stop is pressed`, `Given a track plays`. A scenario can also be written in a fenced block tagged `gherkin`, under its requirement. Its `Scenario:`, `Scenario Outline:` and `Example:` lines are scenarios for coverage, and a trailing `{#S1}` is the anchor that tests link to:

````markdown
### Requirement: Login
The system SHALL sign a user in.

```gherkin
Scenario: Right password {#S1}
  Given an account
  When the password is right
  Then the user is signed in
```
````

A custom profile for requirements can take `style: ears` (or `gherkin`, `user-story`) next to its own sections and fields. A style on any other type is E031.

## Converting between formats

`oxispec adr convert --to madr` rewrites every ADR in the project in another format. Name decisions to convert only those: `oxispec adr convert ADR-0001 --to nygard`. `--dry-run` shows what would change and writes nothing. Running it again changes nothing.

The front matter stays as it is. The body is cut into sections, and sections that mean the same are matched, whatever the format calls them:

| Meaning | Nygard | MADR |
|---|---|---|
| The forces and the question | Context | Context and Problem Statement |
| The choice | Decision | Decision Outcome |
| What follows | Consequences | Consequences, under Decision Outcome |

A section the target has no place for is kept word for word after the others, and the command says so. A section the target requires and the record lacks is added with the placeholder of that format (Considered Options in MADR, Statement in a Y-statement), and the command says that too. A section that only holds a placeholder counts as empty and is not carried over, so converting A to B and back gives the original text. A Y-statement is not written from the other formats; it starts as the skeleton, with the old sections below it for the author to condense.

If a status is not one the target allows (`rejected` in Nygard), the command reports it and keeps the status. `convert` does not change the config; it prints the line to set `formats: adr:` when the project checks another format.

## Importing decisions from other tools

`oxispec import adr --from adr-tools` reads the records of another tool and writes them as ADRs in the chosen format (`--format`, default the one the config chooses, else `default`). The source is `adr-tools`, `madr` or `log4brains`; `--dir` names the folder, which otherwise is where the tool keeps them (`doc/adr`, `docs/decisions`, `docs/adr`).

| Source | Reads |
|---|---|
| adr-tools | `# 1. Title`, `Date:`, and the `## Status` section with its lines such as `Superceded by [2. Title](0002-title.md)`, `Supercedes`, `Amends`, `Amended by`, `Clarifies` and `Clarified by` (the spelling adr-tools uses is accepted, and so is the correct one) |
| MADR | Front matter of 4.0 (`status`, `date`, `decision-makers`), or the bullet lines of 2.x and 3.x (`* Status:`, `* Deciders:`, `* Date:`), with `superseded by [ADR-0005](...)` |
| log4brains | The MADR 2 bullet lines and `Tags:`, in files named by date; ids follow the date order |

The status, date and deciders go to front matter. A link is written once, on the decision that acts: "Superceded by 2" in record 1 and "Supercedes 1" in record 2 both give `supersedes: [ADR-0001]` in record 2, and record 1 becomes `superseded`. The kinds are `supersedes`, `amends` and `clarifies`. Other header lines (`Tags`, `Consulted`, a status line that is none of these) are kept under `## Imported metadata`, so nothing is lost. A link to a record that is not in the folder is reported and left out.

Ids come from the numbers in the file names, plus `--offset`. A project that already has `ADR-0001` refuses the import and says to pass `--offset`. Files whose names do not start with a number (`README.md`, `template.md`) are skipped and listed. Nothing is written if any record cannot be read.

## Importing specs from other tools

`oxispec import specs --from <source>` reads the specs of another tool and writes requirement nodes in the chosen profile (`--format`, default the one the config chooses, else `shall`). `--dry-run` shows the result and writes nothing, and `--json` prints it for a script.

| Source | Where it looks | Becomes |
|---|---|---|
| `openspec` | `openspec/specs/<capability>/spec.md` | `### Requirement:` blocks and `#### Scenario:` steps as they are. `## Purpose` is kept as the intro |
| `speckit` | `specs/<feature>/spec.md` | Each `FR-001: ...` line is a block. Each user story is a block, and its Given, When, Then lines are scenarios |
| `kiro` | `.kiro/specs/<feature>/requirements.md` | Each `### Requirement N` is a block with the user story as its text. Each acceptance criterion is a scenario, split at `THEN` |
| `markdown` | `--dir` names a file or a folder | The shallowest headings under the title are blocks, with their text |
| `prd` | `--dir` names a PRD file or a folder of them | Overview, goals, background and problem sections are the purpose. Each bullet under a section named requirements, features, functional, user stories or scope is a block (`FR-1: ...` is titled by its id, `As a ... I want ...` by the goal). Bullets nested under it, and bullets under an Acceptance criteria heading, are scenarios, split at `THEN`. Other sections (non-goals, open questions, metrics) are listed as left out |
| `openapi` | `--dir` names an OpenAPI or Swagger file (JSON or YAML) or a folder of them | One group of nodes per tag (`api` where an operation has none). Each operation is a node, `GET /orders/{id}`, with its summary as the text, and each response is a scenario. The description is read whole, so it has no plan; `import plan start --from openapi` says so |

The `prd` and `openapi` sources have no default folder, so pass `--dir`. A PRD has no fixed layout, so the section names above are a guess: read the notes in the report and fix the node by hand where the guess was wrong. For an API description, the node is a start. It records which operations exist and what each answers; the behavior behind them is for you or an agent to write.

`--dir` replaces the default folder. Each requirement (the *blocks* of the table) becomes a node of its own, `REQ-<NAME>-NNNN` in `specs/requirements/<name>/`, where the name is the capability or feature folder (or the file name) in capitals, without a leading number: `001-user-auth` gives `REQ-USERAUTH-0001`, `REQ-USERAUTH-0002` and on in the order of the file. Each node is a file named after its requirement (`deleting-moves-a-post-to-the-bin.md`; a long title is cut, and a name that is taken gets `-2`). The purpose of the spec is kept in the first node. Scenarios are numbered from `{#S1}` in each node, so one is linked as `REQ-USERAUTH-0002.S1`. **The ID of the source is kept** for OpenSpec and plain markdown, when the title of a requirement begins with one (`### Requirement: BIN-1 Deleting ...`): the node is `REQ-BIN-1` (the prefix is the domain, the number is the one written, with no zeros added), `BIN-1` is its alias, so `show BIN-1`, a link, or `verifies: BIN-1.S1` in a test find it, and the title keeps the ID. Capabilities may share a prefix (`ANAL-` in three specs): their requirements are all `REQ-ANAL-n`, in the folder of their capability. If a number is claimed twice, the second node gets the next free one and the import says so; a requirement with no ID in a file whose other requirements have one gets a number nobody asked for. Spec Kit, Kiro and a PRD number from the start in every document (`FR-001`), so they keep the numbering by capability name. A delta of an open change that names `BIN-1 ...` links that requirement. `--id-scheme random` gives a random suffix in place of the number, and the alias stays.

Whatever cannot be mapped is listed, never dropped quietly: design notes, tasks, plans, open OpenSpec changes (start a change set for them), and sections such as Spec Kit's Key Entities. A file that already has an `id` is an OxiSpec node and is skipped. Where the source has no scenario for a requirement, a placeholder scenario is written, and where a statement has no SHALL or MUST the profile needs one, a sentence with SHALL is put before it. Each such step is a note on the node in the report, so `oxispec check` and the notes together show what is left to fill in. The command stops before writing if a target file already exists.

### Deciding file by file

`import specs` takes the specs only. To bring in the rest, and to choose what each file becomes, use a plan:

```bash
oxispec import plan start --from openspec     # writes .oxispec/import-plan.json and lists the items
oxispec import plan show 2 --draft            # the file item 2 would write, and what is left to fill in
oxispec import plan set 2 --kind adr          # requirement, change, adr or skip; --title renames it
oxispec import apply --dry-run                # every file that would be written
oxispec import apply                          # write them and remove the plan
```

An item is a spec (proposed as a requirement node), the design, tasks and plan files next to a spec (a change set), or an open change (a change set from its proposal, design, tasks and spec deltas), and archived changes (skipped). A change set is `draft`, with `Why`, `What changes` and `Impact` from the proposal, then `Design`, `Tasks` and one `Delta` section for each capability. The requirements that a delta modifies or removes are linked with `changes`, each to the node of that requirement, when it exists or is part of the same import. The ones it adds are listed and not linked. Headings in the copied text sit two levels deeper, so a `### Requirement:` in a delta is not read as a requirement of the change set. Nothing is written until `apply`, and it stops without writing if a target file exists.

The `oxispec-import-specs` skill walks an agent through this: it proposes, asks the person about each item that could be something else, shows the drafts and the dry run, and applies only after a yes.

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
- Converting and importing work on ADRs. Other types come later (iteration 29).
- `new` writes templates for ADRs and requirements. Other types have none.
- A style checks one requirement at a time. It does not judge whether the requirement is a good one.
- A running `oxispec mcp` or `oxispec serve` notices a change to the config within a moment, like any other edit.
