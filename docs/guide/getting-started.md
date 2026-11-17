# Getting started

About ten minutes. You end with a project that has a requirement, a decision, a link between them and a clean `oxispec check`.

## Install

Build from source (Rust 1.89 or newer, and a C compiler). Ready-made binaries and the other ways to install come with the first release, and are in the [README](../../README.md#install):

```bash
git clone https://github.com/alexrgb/oxispec
cd oxispec
cargo install --path crates/oxispec-cli
oxispec --version
```

## Create a project

Go to the folder of the project that you want to specify, normally a git repository, and start:

```bash
oxispec init --preset standard
```

This creates `.oxispec/config.yaml` and the folders for the modules in the preset. `standard` turns on requirements, tests, decision records, the architecture model, state machines, claims, tech debt, CI reports, agent skills and the MCP server. `minimal` (the default) is only the core, and `full` also adds releases and the rest. Releases need `oxispec modules add releases` on a `standard` project. You can add or remove modules later with `oxispec modules add <name>`.

`init` also writes skill files for AI clients (`.claude/skills`, `.cursor/skills`, and others). They teach an agent the commands. If you do not want them, run `oxispec init --preset standard --agents none`, or delete the folders.

Then fix which documents the project keeps and in what format, so that nobody has to be asked later:

```bash
oxispec onboard                       # asks about each kind of document
oxispec onboard --docs requirement:ears,adr:madr,container:c4-container,risk
oxispec onboard --show                # what the project keeps, with the command for each
```

`onboard` writes the kinds to `documents` in the config and their formats to `formats`, turns on the modules they need, and starts the project when there is none. After that `oxispec new adr "Title"` writes a MADR record without `--format`, and `new` refuses a kind that is not on the list. `--yes` asks nothing and keeps what the project has (requirements, ADRs and change sets for a new project). Changing a format later holds the records that exist to the new one, so `check` may report the ones that lack its sections.

Commit `.oxispec/config.yaml` and your specs. `.oxispec/cache` is a local index that you can ignore.

## A monorepo with several products

Give each product its own project and its own formats: `cd apps/a && oxispec onboard --docs requirement:ears,adr:madr`, then `cd ../b && oxispec onboard --docs requirement:gherkin,adr:nygard`. A command run inside a product folder uses that product's config, IDs and formats, so two products can both have `REQ-AUTH-0001`.

At the top, a project of its own is optional. If there is one, it leaves the files of the products out of its graph, `oxispec products` lists them, `oxispec check` goes through every product with its own config (a finding names the product in its path), and `oxispec site` builds one site: a folder for each product, a switcher between products on every page, and a page at the top that lists them. A site of products is built from the working tree; `--at` and `--releases` are for a project without products. Links between products are described below.

### A platform and the products that depend on it

A product can link to a node of another product with `product:ID`, where `product` is the folder of that project below the top project: `oxispec link REQ-UI-0001 satisfies platform:REQ-AUTH-0001` (run in `apps/a`, with `platform/` a project of its own). The top project is needed, because the namespace is a path from it. `check` reads the link in the other project: a node that does not exist is E002, a scenario that does not is E009, so a change in the platform that breaks a product shows up as an error in the product. A namespace that is no project stays an unchecked reference (I001). With `--at`, the link is read against the other project as it was at that revision (when it is in the same repository and was a project then); otherwise it stays I001.

To see who depends on a node, run `oxispec show REQ-AUTH-0001` in the platform: it ends with `linked from other products`. In a pull request, `oxispec ci` in the platform lists the nodes of other products that link to the nodes the change touches (`dependents` in `--json`, and a section in the Markdown report), so the review sees which products to check. On the site, a link to another product leads to its page and the page of a node lists the nodes of other products that use it.

## Your first requirement

```bash
oxispec new requirement auth "Admin session timeout"
```

```text
created REQ-AUTH-0001 (oxispec/requirements/auth/admin-session-timeout.md)
```

Open the file. The command wrote a template:

```markdown
---
id: REQ-AUTH-0001
type: requirement
status: draft
---
### Requirement: Admin session timeout
The system SHALL <describe the behavior>.

#### Scenario: <name> {#S1}
- **WHEN** <condition>
- **THEN** <outcome>
```

Fill it in:

```markdown
### Requirement: Admin session timeout
The system SHALL expire an admin session after 15 minutes of inactivity.

#### Scenario: Idle admin {#S1}
- **WHEN** an admin does nothing for 15 minutes
- **THEN** the session is invalidated
```

Two rules matter: the description has `SHALL` or `MUST` in capitals, and there is at least one scenario. `{#S1}` is an anchor, so that tests and other nodes can point at this scenario.

## A decision, and a link

```bash
oxispec new adr "Use Postgres for sessions"
oxispec link REQ-AUTH-0001 decided-by ADR-0001
```

Fill in the decision record (`oxispec/adr/0001-use-postgres-for-sessions.md`) with the context and the choice. `link` writes the link into the front matter of the first node:

```yaml
links:
  decided-by:
    - ADR-0001
```

Both ends must exist. `oxispec show REQ-AUTH-0001` lists the node with its links, and `oxispec show ADR-0001` lists what links to it. Links are written on one side only; the other side is computed.

## Check

```bash
oxispec check
```

```text
2 nodes checked: 0 errors, 0 warnings
```

`check` exits with code 1 if there are errors, so it fits into CI. It catches mistakes such as these:

```text
error[E002]: dangling link: `decided-by` points to ADR-0099, which does not exist
error[E007]: requirement "Admin session timeout" needs SHALL or MUST in its description
```

The code (`E002`) is described in the table at the end of [format](../format.md). Fix the file and run it again. `oxispec check --changed` looks only at what differs from git `HEAD`, and what links to it, which is quick in a large project.

## What next

- `oxispec status` summarizes the project: how many nodes of each kind, and what needs attention.
- `oxispec search session timeout` searches the text of all nodes.
- Read [Concepts](concepts.md) for the model, then [Writing requirements](requirements.md).
- To publish the specs as a website: `oxispec site --out public`.
