# Getting started

About ten minutes. You end with a project that has a requirement, a decision, a link between them and a clean `oxispec check`.

## Install

Releases are not published yet, so build from source (Rust 1.89 or newer):

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

Commit `.oxispec/config.yaml` and your specs. `.oxispec/cache` is a local index that you can ignore.

## Your first requirement

```bash
oxispec new requirement auth "Admin session timeout"
```

```text
created REQ-AUTH-0001 (specs/requirements/auth/admin-session-timeout.md)
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

Fill in the decision record (`specs/adr/0001-use-postgres-for-sessions.md`) with the context and the choice. `link` writes the link into the front matter of the first node:

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
