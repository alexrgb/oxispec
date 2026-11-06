# Command reference

Written by `oxispec docs reference` from the program itself. Do not edit it by hand.

Every command also accepts `--root <ROOT>` (the project folder; default: the nearest parent with `.oxispec/config.yaml`), `--no-cache` (do not read or write the local index) and `--help`.

Commands: [`init`](#oxispec-init), [`modules`](#oxispec-modules), [`new`](#oxispec-new), [`change`](#oxispec-change), [`release`](#oxispec-release), [`agents`](#oxispec-agents), [`status`](#oxispec-status), [`connect`](#oxispec-connect), [`context`](#oxispec-context), [`claims`](#oxispec-claims), [`architecture`](#oxispec-architecture), [`machine`](#oxispec-machine), [`mcp`](#oxispec-mcp), [`serve`](#oxispec-serve), [`debt`](#oxispec-debt), [`compliance`](#oxispec-compliance), [`docs`](#oxispec-docs), [`adr`](#oxispec-adr), [`import`](#oxispec-import), [`formats`](#oxispec-formats), [`token`](#oxispec-token), [`link`](#oxispec-link), [`check`](#oxispec-check), [`ci`](#oxispec-ci), [`site`](#oxispec-site), [`trace`](#oxispec-trace), [`suspect`](#oxispec-suspect), [`confirm`](#oxispec-confirm), [`results`](#oxispec-results), [`coverage`](#oxispec-coverage), [`search`](#oxispec-search), [`history`](#oxispec-history), [`rollback`](#oxispec-rollback), [`tag`](#oxispec-tag), [`diff`](#oxispec-diff), [`index`](#oxispec-index), [`show`](#oxispec-show)

## `oxispec init`

Create .oxispec/config.yaml and the folders for the enabled modules

```text
Usage: oxispec init [OPTIONS]
```

- `--preset <PRESET>`: minimal, specs-tests, docs, standard or full. Default: `minimal`.
- `--with <WITH>`: Extra modules to enable, comma separated.
- `--without <WITHOUT>`: Modules to leave out of the preset, comma separated.
- `--id-scheme <ID_SCHEME>`: How new IDs are numbered. Use random when several branches add specs at once. One of `sequential`, `random`. Default: `sequential`.
- `--agents <AGENTS>`: AI clients that get skills when the agents module is on, comma separated: claude, cursor, copilot, gemini, agents, or none (default: claude,cursor,copilot,gemini).

## `oxispec modules`

List, add or remove modules

```text
Usage: oxispec modules [OPTIONS] [COMMAND]
```

Subcommands: `list`, `add`, `remove`

### `oxispec modules list`

List all modules (default)

```text
Usage: oxispec modules list [OPTIONS]
```

### `oxispec modules add`

Enable modules, and the modules they need

```text
Usage: oxispec modules add [OPTIONS] [NAMES]...
```

- `<NAMES>`

### `oxispec modules remove`

Disable modules. Existing files are never deleted

```text
Usage: oxispec modules remove [OPTIONS] [NAMES]...
```

- `<NAMES>`

## `oxispec new`

Create a node from a template

```text
Usage: oxispec new [OPTIONS] <COMMAND>
```

- `--id-scheme <ID_SCHEME>`: Override the project's ID scheme for this node. One of `sequential`, `random`.

Subcommands: `requirement`, `adr`, `debt`, `prd`, `change`, `vocabulary`, `machine`, `architecture`

### `oxispec new requirement`

A requirement with one scenario, for example: new requirement auth "Admin session timeout"

```text
Usage: oxispec new requirement [OPTIONS] <DOMAIN> <TITLE>
```

- `<DOMAIN>`
- `<TITLE>`
- `--format <FORMAT>`: The format profile to write it in: shall, ears, gherkin, user-story or your own. Default: the one the config chooses for requirements, else shall.
- `--id-scheme <ID_SCHEME>`: Override the project's ID scheme for this node. One of `sequential`, `random`.

### `oxispec new adr`

An architecture decision record

```text
Usage: oxispec new adr [OPTIONS] <TITLE>
```

- `<TITLE>`
- `--format <FORMAT>`: The format profile to write it in. Default: the one the config chooses for ADRs.
- `--id-scheme <ID_SCHEME>`: Override the project's ID scheme for this node. One of `sequential`, `random`.

### `oxispec new debt`

A tech debt item, for example: new debt "Retry loop has no backoff" --from REQ-API-0003

```text
Usage: oxispec new debt [OPTIONS] <TITLE>
```

- `<TITLE>`
- `--severity <SEVERITY>`: low, medium, high or critical. Default: `medium`.
- `--from <FROM>`: The node it arises from; becomes an `arises-from` link.
- `--id-scheme <ID_SCHEME>`: Override the project's ID scheme for this node. One of `sequential`, `random`.

### `oxispec new prd`

A product requirements document. Requirements that `refine` it make up the document `docs prd` writes

```text
Usage: oxispec new prd [OPTIONS] <TITLE>
```

- `<TITLE>`
- `--id-scheme <ID_SCHEME>`: Override the project's ID scheme for this node. One of `sequential`, `random`.

### `oxispec new change`

A change set: a proposed change to the specs, with a lifecycle

```text
Usage: oxispec new change [OPTIONS] <TITLE>
```

- `<TITLE>`
- `--id-scheme <ID_SCHEME>`: Override the project's ID scheme for this node. One of `sequential`, `random`.

### `oxispec new vocabulary`

A vocabulary: the typed variables that claims in requirements talk about

```text
Usage: oxispec new vocabulary [OPTIONS] <TITLE>
```

- `<TITLE>`
- `--id-scheme <ID_SCHEME>`: Override the project's ID scheme for this node. One of `sequential`, `random`.

### `oxispec new machine`

A state machine: states, events and transitions, with properties that are checked

```text
Usage: oxispec new machine [OPTIONS] <TITLE>
```

- `<TITLE>`
- `--id-scheme <ID_SCHEME>`: Override the project's ID scheme for this node. One of `sequential`, `random`.

### `oxispec new architecture`

An element of the architecture model, or a deployment environment

```text
Usage: oxispec new architecture [OPTIONS] <KIND> <TITLE>
```

- `<KIND>`
- `<TITLE>`
- `--id-scheme <ID_SCHEME>`: Override the project's ID scheme for this node. One of `sequential`, `random`.

## `oxispec change`

Work with change sets

```text
Usage: oxispec change [OPTIONS] <COMMAND>
```

Subcommands: `status`, `list`

### `oxispec change status`

Move a change set along draft, in-review, approved, merged, reverted

```text
Usage: oxispec change status [OPTIONS] <ID> <STATUS>
```

- `<ID>`
- `<STATUS>`
- `--by <BY>`: Who approves, when moving to approved (default: git user.name).
- `--force`: Skip a step, or set a status that is not the next one.

### `oxispec change list`

List change sets

```text
Usage: oxispec change list [OPTIONS]
```

- `--status <STATUS>`: Only change sets with this status.
- `--json`

## `oxispec release`

Prepare, check and publish releases of the specs

```text
Usage: oxispec release [OPTIONS] <COMMAND>
```

Subcommands: `prepare`, `check`, `publish`, `list`

### `oxispec release prepare`

Work out the version and notes since the last release, and write the manifest

```text
Usage: oxispec release prepare [OPTIONS] [VERSION]
```

- `--force`: Write the notes again for a version that already has a manifest.
- `--dry-run`: Show the manifest and write nothing.
- `--json`

### `oxispec release check`

Run the gates for a release. Exits with 1 when it is not ready

```text
Usage: oxispec release check [OPTIONS] <VERSION>
```

- `--json`

### `oxispec release publish`

Run the gates on the last commit and tag it specs-vVERSION

```text
Usage: oxispec release publish [OPTIONS] <VERSION>
```

- `--json`

### `oxispec release list`

List releases and whether they are published

```text
Usage: oxispec release list [OPTIONS]
```

- `--json`

## `oxispec agents`

Skills for AI clients: write them, check them for drift, or export them for other tools

```text
Usage: oxispec agents [OPTIONS] <COMMAND>
```

Subcommands: `list`, `update`, `check`, `export`

### `oxispec agents list`

List the clients and the skills, and which ones this project gets

```text
Usage: oxispec agents list [OPTIONS]
```

- `--json`

### `oxispec agents update`

Write the skills into each client's folder. A file edited by hand is left alone

```text
Usage: oxispec agents update [OPTIONS]
```

- `--client <CLIENTS>`: Only these clients (default: the project's, set by `init --agents`).
- `--force`: Replace files that were edited by hand.
- `--dry-run`: Show what would be written and write nothing.

### `oxispec agents check`

Fail when a skill file is missing, out of date or edited by hand

```text
Usage: oxispec agents check [OPTIONS]
```

- `--client <CLIENTS>`
- `--json`

### `oxispec agents export`

Write the skills in the layout another tool reads

```text
Usage: oxispec agents export [OPTIONS] --to <TO>
```

- `--to <TO>`: skills-repo (`npx skills add`) or ruler. One of `skills-repo`, `ruler`. Required.
- `--out <OUT>`: The folder to write into. Default: `.`.
- `--all`: Every skill and no project notes, without needing a project.

## `oxispec status`

A summary of the project: what is in it and what needs attention

```text
Usage: oxispec status [OPTIONS]
```

- `--at <AT>`: Read the specs as they were at a git revision.
- `--json`

## `oxispec connect`

Add the MCP server to the config files of AI clients (claude, cursor, gemini, copilot)

```text
Usage: oxispec connect [OPTIONS] [CLIENTS]...
```

- `<CLIENTS>`: The clients to write for (default: the project's `agents`, else all four).
- `--read-only`: Start the server with `--read-only`.
- `--url <URL>`: Point the clients at a running `oxispec serve`, for example `https://specs.example.com/mcp`. The token is read from the OXISPEC_TOKEN environment variable, and is never written to a file.
- `--dry-run`: Show what would be written, and write nothing.

## `oxispec context`

The text of some nodes and what is near them, sized to a token budget, for an agent

```text
Usage: oxispec context [OPTIONS] <IDS>...
```

- `<IDS>`: The nodes to start from.
- `--depth <DEPTH>`: How many link hops from them to look. Default: `2`.
- `--budget <BUDGET>`: Tokens to fill, counted as one per four characters. Default: `8000`.
- `--at <AT>`: Read the specs as they were at a git revision.
- `--json`

## `oxispec claims`

List the vocabulary and the claims written against it. `check` reports conflicts

```text
Usage: oxispec claims [OPTIONS]
```

- `--var <NAME>`: Only this variable.
- `--at <AT>`: Read the specs as they were at a git revision.
- `--json`

## `oxispec architecture`

The architecture model: list its elements, or draw a view of it as Mermaid

```text
Usage: oxispec architecture [OPTIONS] [COMMAND]
```

- `--at <AT>`: Read the specs as they were at a git revision.
- `--json`

Subcommands: `list`, `view`

### `oxispec architecture list`

List the elements, environments and rules (the default)

```text
Usage: oxispec architecture list [OPTIONS]
```

- `--at <AT>`: Read the specs as they were at a git revision.
- `--json`

### `oxispec architecture view`

Draw one view as a Mermaid flowchart

```text
Usage: oxispec architecture view [OPTIONS] <KIND> <ID>
```

- `<KIND>`
- `<ID>`: The system, container or environment the view is of.
- `--at <AT>`: Read the specs as they were at a git revision.
- `--json`

## `oxispec machine`

List state machines, or show one with its properties, a diagram or generated tests

```text
Usage: oxispec machine [OPTIONS] [ID]
```

- `<ID>`: A state machine ID, such as SM-ORDER. Without it, every machine is listed.
- `--diagram`: Print a Mermaid state diagram.
- `--tests`: Print one test for each transition: the path from the initial state to it.
- `--limit <LIMIT>`: With --tests, print only this many.
- `--at <AT>`: Read the specs as they were at a git revision.
- `--json`

## `oxispec mcp`

Serve the specs to AI clients over the Model Context Protocol, on stdin and stdout

```text
Usage: oxispec mcp [OPTIONS]
```

- `--read-only`: Do not offer the tool that writes a draft change set.

## `oxispec serve`

Serve the specs to many AI clients over HTTP, read-only, with one bearer token per client

```text
Usage: oxispec serve [OPTIONS]
```

- `--listen <ADDRESS>`: Address to listen on. Default: `127.0.0.1:7878`.
- `--behind-tls-proxy`: Allow a non-local address because a TLS proxy in front encrypts the traffic.
- `--audit-log <FILE>`: Where to append one JSON line per request. Default: .oxispec/server/audit.jsonl.

## `oxispec debt`

The tech debt backlog, the most serious and the oldest first

```text
Usage: oxispec debt [OPTIONS]
       oxispec debt <COMMAND>
```

- `--status <STATUS>`: Only items with this status: open, planned, in-progress, resolved or accepted.
- `--severity <SEVERITY>`: Only items with this severity: low, medium, high or critical.
- `--all`: Include items that are resolved or accepted.
- `--json`

Subcommands: `status`

### `oxispec debt status`

Move an item to a status, and record today as the day it was reviewed

```text
Usage: oxispec debt status [OPTIONS] <ID> <STATUS>
```

- `<ID>`
- `<STATUS>`

## `oxispec compliance`

Traceability and sign-offs the project's compliance rules ask for. Exits 1 when one is missing

```text
Usage: oxispec compliance [OPTIONS]
       oxispec compliance <COMMAND>
```

- `--json`

Subcommands: `standards`

### `oxispec compliance standards`

The built-in standards, their levels and the rules each level adds

```text
Usage: oxispec compliance standards [OPTIONS]
```

- `--json`

## `oxispec docs`

Documentation written from the program: the command reference and the check codes

```text
Usage: oxispec docs [OPTIONS] <COMMAND>
```

Subcommands: `reference`, `openapi`, `prd`

### `oxispec docs reference`

Write commands.md and check-codes.md. With --check, write nothing and exit 1 when they differ

```text
Usage: oxispec docs reference [OPTIONS]
```

- `--out <OUT>`: Where to write them. Default: `docs/reference`.
- `--check`

### `oxispec docs openapi`

An OpenAPI skeleton made from the requirements that are titled as calls, such as `GET /orders/{id}`. It has no schemas or security. With --against, compare a file with the requirements instead and exit 1 when they differ

```text
Usage: oxispec docs openapi [OPTIONS]
```

- `--out <OUT>`: Write the skeleton to this file instead of printing it.
- `--against <AGAINST>`: Compare this OpenAPI or Swagger file (JSON or YAML) with the requirements.
- `--title <TITLE>` Default: `API`.

### `oxispec docs prd`

The product requirements document a `prd` node and the requirements that refine it make. Printed, or written with --out. It is a view of the nodes: change them, not the file

```text
Usage: oxispec docs prd [OPTIONS] <ID>
```

- `<ID>`
- `--out <OUT>`: Write the document to this file instead of printing it.

## `oxispec adr`

Decision records: convert them between formats

```text
Usage: oxispec adr [OPTIONS] <COMMAND>
```

Subcommands: `convert`

### `oxispec adr convert`

Rewrite decisions in another format. The front matter stays; sections that mean the same are matched, the others are kept as they are. Example: adr convert --to madr ADR-0001

```text
Usage: oxispec adr convert [OPTIONS] --to <TO> [IDS]...
```

- `<IDS>`: The decisions to convert. Default: every ADR in the project.
- `--to <TO>`: The format profile to convert to: nygard, madr, y-statement, or one of your own. Required.
- `--dry-run`: Show what would change and write nothing.
- `--json`

## `oxispec import`

Bring in records written for another tool

```text
Usage: oxispec import [OPTIONS] <COMMAND>
```

Subcommands: `adr`, `plan`, `apply`, `specs`

### `oxispec import adr`

Records of adr-tools, MADR or log4brains. Links such as "Superceded by" and "Amended by" become `supersedes`, `amends` and `clarifies` links

```text
Usage: oxispec import adr [OPTIONS] --from <FROM>
```

- `--from <FROM>`: adr-tools, madr or log4brains. Required.
- `--dir <DIR>`: The folder of records. Default: where the tool keeps them (doc/adr, docs/decisions, docs/adr).
- `--format <FORMAT>`: The format profile to write them in. Default: the one the config chooses for ADRs.
- `--offset <OFFSET>`: Add this to every number, when the project has decisions already. Default: `0`.
- `--dry-run`: Show what would be written and write nothing.
- `--json`

### `oxispec import plan`

Decide what to make of each file of another tool, look at drafts, then apply

```text
Usage: oxispec import plan [OPTIONS] <COMMAND>
```

Subcommands: `start`, `show`, `set`

#### `oxispec import plan start`

Look at the files and propose what to make of each. Writes .oxispec/import-plan.json

```text
Usage: oxispec import plan start [OPTIONS] --from <FROM>
```

- `--from <FROM>`: openspec, speckit, kiro, markdown, prd (a product requirements document) or openapi (OpenAPI or Swagger, JSON or YAML). Required.
- `--dir <DIR>`
- `--force`: Start again, replacing a plan that exists.
- `--json`

#### `oxispec import plan show`

The items of the plan, or one item. With --draft: the file it would write

```text
Usage: oxispec import plan show [OPTIONS] [N]
```

- `<N>`
- `--draft`
- `--json`

#### `oxispec import plan set`

Change what an item becomes: requirement, change, adr or skip

```text
Usage: oxispec import plan set [OPTIONS] <N>
```

- `<N>`
- `--kind <KIND>`
- `--title <TITLE>`

### `oxispec import apply`

Write the nodes of the import plan. With --dry-run: show them and write nothing

```text
Usage: oxispec import apply [OPTIONS]
```

- `--dry-run`
- `--json`

### `oxispec import specs`

Specs of OpenSpec, Spec Kit, Kiro or plain markdown. Each file becomes a requirement node; what has no place in one (design notes, tasks, open changes) is listed

```text
Usage: oxispec import specs [OPTIONS] --from <FROM>
```

- `--from <FROM>`: openspec, speckit, kiro, markdown, prd (a product requirements document) or openapi (OpenAPI or Swagger, JSON or YAML). Required.
- `--dir <DIR>`: The folder of specs (for markdown, a file or folder). Default: where the tool keeps them.
- `--format <FORMAT>`: The requirement profile to write them in. Default: the one the config chooses.
- `--id-scheme <ID_SCHEME>`: sequential or random; overrides the config. One of `sequential`, `random`.
- `--dry-run`: Show what would be written and write nothing.
- `--json`

## `oxispec formats`

The format profiles a project can use: what a node of a type must contain

```text
Usage: oxispec formats [OPTIONS] <COMMAND>
```

Subcommands: `list`, `show`

### `oxispec formats list`

List the profiles, built in and the project's own

```text
Usage: oxispec formats list [OPTIONS]
```

- `--json`

### `oxispec formats show`

Show what a profile requires and the template `new` writes

```text
Usage: oxispec formats show [OPTIONS] <NAME>
```

- `<NAME>`
- `--json`

## `oxispec token`

Create, list and revoke the tokens that `serve` accepts

```text
Usage: oxispec token [OPTIONS] <COMMAND>
```

Subcommands: `add`, `list`, `revoke`

### `oxispec token add`

Create a token and print it once. The project keeps only a hash

```text
Usage: oxispec token add [OPTIONS] <NAME>
```

- `<NAME>`: A name for the client, such as `ci` or `docs-bot`.
- `--types <TYPES>`: Limit the token to these node types, for example `requirement,adr`. It then reads only those nodes, with spec_search and spec_get. Without it, it reads everything.
- `--json`

### `oxispec token list`

List token names and what each may read. Secrets are never shown

```text
Usage: oxispec token list [OPTIONS]
```

- `--json`

### `oxispec token revoke`

Stop accepting a token

```text
Usage: oxispec token revoke [OPTIONS] <NAME>
```

- `<NAME>`

## `oxispec link`

Add a typed link from one node to another, for example: link REQ-AUTH-0001 decided-by ADR-0001

```text
Usage: oxispec link [OPTIONS] <FROM> <KIND> <TO>
```

- `<FROM>`: The node that holds the link.
- `<KIND>`: satisfies, verifies, implements, decided-by, supersedes, refines, governed-by, describes, overrides, conflicts-with, depends-on, changes, includes, amends or clarifies.
- `<TO>`: A node, or a scenario of a node such as REQ-AUTH-0001.S1.

## `oxispec check`

Run structural checks. Exit code 1 when there are errors

```text
Usage: oxispec check [OPTIONS]
```

- `--json`
- `--strict`: Treat warnings as errors.
- `-v, --verbose`: Also print informational findings.
- `--changed`: Only report findings for spec files that changed since BASE, and the nodes that link to them.
- `--base <BASE>`: The git ref to compare with, for --changed. Default: `HEAD`.
- `--at <AT>`: Check the specs as they were at a git revision (a branch, tag, hash or date:YYYY-MM-DD).

## `oxispec ci`

Check a change for CI: only the findings it introduces, with reports for pull requests

```text
Usage: oxispec ci [OPTIONS]
```

- `--base <BASE>`: What to compare with (the branch point of HEAD and BASE is used). Default: `origin/main`.
- `--strict`: Warnings that the change introduces fail the run too.
- `--json`
- `--sarif <FILE>`: Write SARIF (for code scanning) to this file.
- `--junit <FILE>`: Write JUnit XML to this file.
- `--markdown <FILE>`: Write the pull request comment (markdown) to this file.

## `oxispec site`

Build a static site: a page per node, linked by ID

```text
Usage: oxispec site [OPTIONS]
```

- `--out <OUT>`: The folder to write to. Default: `site`.
- `--at <AT>`: Build the specs as they were at a git revision (a branch, tag, hash or date:YYYY-MM-DD).
- `--title <TITLE>`: The site's name (default: the project folder's name).
- `--releases`: Build the working tree as `latest` and a folder for every spec tag, with a version picker.

## `oxispec trace`

Show where a node is implemented and tested, from `spec:` and `verifies:` comments

```text
Usage: oxispec trace [OPTIONS] [ID]
```

- `<ID>`: A node (default: list every annotation found).
- `--json`

## `oxispec suspect`

List links between specs and code that changed since they were confirmed

```text
Usage: oxispec suspect [OPTIONS]
```

- `--all`: Also list links that are fine.
- `--strict`: Links that are not confirmed yet, or whose code is gone, fail the run too.
- `--json`

## `oxispec confirm`

Record that the code and the spec still agree, so their links stop being suspect

```text
Usage: oxispec confirm [OPTIONS] [IDS]...
```

- `<IDS>`: Nodes whose links to confirm.
- `--all`: Confirm every link.
- `--dry-run`: Show what would be confirmed and write nothing.

## `oxispec results`

Import test results (JUnit XML or `go test -json`), so coverage can say what passes

```text
Usage: oxispec results [OPTIONS] <COMMAND>
```

Subcommands: `import`, `show`, `clear`

### `oxispec results import`

Read results from files and keep them in the cache

```text
Usage: oxispec results import [OPTIONS] <FILES>...
```

- `<FILES>`
- `--replace`: Forget earlier imports first.

### `oxispec results show`

Show the imported results

```text
Usage: oxispec results show [OPTIONS]
```

- `--json`

### `oxispec results clear`

Forget the imported results

```text
Usage: oxispec results clear [OPTIONS]
```

## `oxispec coverage`

Report requirements nothing implements and scenarios no test verifies

```text
Usage: oxispec coverage [OPTIONS]
```

- `--status <STATUS>`: Only requirements in this status, since a draft is not built yet.
- `--fail-under <PERCENT>`: Exit with 1 when either percentage is below this.
- `--require-pass`: Count a scenario only when its tests passed in the imported results, and fail on a failing one.
- `--all`: List everything, not the first 20.
- `--json`

## `oxispec search`

Search the text of all nodes, for example: search "session timeout" --type requirement

```text
Usage: oxispec search [OPTIONS] <QUERY>...
```

- `<QUERY>`: The words to look for. All of them have to match. Quotes make a phrase.
- `--type <TYPE>`: Only nodes of this type. Repeat for several types.
- `--status <STATUS>`: Only nodes with this status.
- `-n, --limit <LIMIT>`: How many results to show. Default: `10`.
- `--rebuild`: Delete the stored search index and build it again.
- `--json`

## `oxispec history`

Show how a node changed over time, from git history

```text
Usage: oxispec history [OPTIONS] <ID>
```

- `<ID>`
- `--at <AT>`: Look at history up to this revision, and find nodes that no longer exist.
- `--limit <LIMIT>`: Number of commits to show. Default: `20`.
- `--json`

## `oxispec rollback`

Put nodes back to how they were at a git revision, after checking nothing breaks. Nothing is committed: review the result and commit it

```text
Usage: oxispec rollback [OPTIONS] <REVISION> [IDS]...
```

- `<REVISION>`: A branch, tag, hash, or date:YYYY-MM-DD.
- `<IDS>`: The nodes to restore.
- `--all`: Restore every node that differs from that revision.
- `--force`: Go ahead although it would introduce errors or lose uncommitted work.
- `--dry-run`: Show what would happen and change nothing.
- `--json`

## `oxispec tag`

Name a revision with a tag (specs-NAME), or list the tags when no name is given

```text
Usage: oxispec tag [OPTIONS] [NAME]
```

- `<NAME>`
- `--at <AT>`: The revision to tag (default: HEAD).
- `-m, --message <MESSAGE>`: The tag's message.
- `--json`

## `oxispec diff`

Show how the specs differ between two revisions (default for the second: the working tree)

```text
Usage: oxispec diff [OPTIONS] <FROM> [TO]
```

- `<FROM>`: A branch, tag, hash, or date:YYYY-MM-DD.
- `<TO>`
- `--json`

## `oxispec index`

Build or refresh the local index in .oxispec/cache

```text
Usage: oxispec index [OPTIONS]
```

- `--rebuild`: Delete the cache first, so every file is parsed again.
- `--json`

## `oxispec show`

Show a node, its links, and the nodes that link to it

```text
Usage: oxispec show [OPTIONS] <ID>
```

- `<ID>`
- `--json`
- `--at <AT>`: Show the node as it was at a git revision (a branch, tag, hash or date:YYYY-MM-DD).
