# Writing requirements

## A requirement and its scenarios

A requirement node holds one or more `### Requirement:` blocks. Each block has a statement with `SHALL` or `MUST` in capitals, and one or more `#### Scenario:` blocks with `WHEN` and `THEN` steps (and `GIVEN` and `AND` if you want them). Give a scenario an anchor, `{#S1}`, to make it addressable.

```bash
oxispec new requirement billing "Invoice rounding"
```

Keep a requirement small and testable. One behavior, with the scenarios that show its edges: the normal case, the limit, the failure.

### Choosing a style

The default style is SHALL with scenarios. Other styles are available as format profiles: `ears`, `gherkin` and `user-story`.

```bash
oxispec formats list
oxispec new requirement billing "Invoice rounding" --format ears
```

To use one for the whole project, set it in `.oxispec/config.yaml`:

```yaml
formats:
  requirement: ears
```

The style changes what `check` expects of the wording (for EARS, a pattern such as "When ..., the system shall ..."). It does not change the links or any other command. See [profiles](../profiles.md#requirement-styles).

## Status

A requirement starts as `draft`. Change the `status:` line when it moves on (`proposed`, `accepted`, and so on, as your profile allows). `oxispec coverage --status accepted` can then ignore drafts, which are not expected to be built yet.

## Decisions

Record a hard-to-reverse decision as an ADR, and link the requirements it explains:

```bash
oxispec new adr "Use Postgres for sessions"
oxispec link REQ-AUTH-0001 decided-by ADR-0001
```

When a decision replaces an earlier one, link `supersedes` from the new one; the old one is then shown as superseded. ADRs can be written as MADR, Nygard and others, and converted between them: `oxispec adr convert --help`.

## Changing specs with a change set

To change a requirement that others rely on, propose the change first.

```bash
oxispec new change "Tighten timeouts"
oxispec link CHG-0001 changes REQ-AUTH-0001
oxispec change status CHG-0001 in-review
oxispec change status CHG-0001 approved --by "Ann Lee"
oxispec change status CHG-0001 merged
```

A change set has a lifecycle (`draft`, `in-review`, `approved`, `merged`, `reverted`) and records who approved it and when. The approval is a record in the file, kept in git history; your repository's review rules are what stop someone approving their own change.

## Review in CI

```bash
oxispec ci --base origin/main
```

reports only the findings that the branch introduces, what changed in the graph and what it affects, as a report for a pull request. See [CI](../ci.md).

## Releases of the specs

`oxispec release prepare` lists what changed since the last release and suggests a version (breaking, added, changed). `oxispec release check` runs the gates and `oxispec release publish 0.2.0` tags the specs. Needs the `releases` module. See [releasing](../releasing.md).

## With an AI agent

`oxispec init` writes skills that tell an agent how to explore, propose, apply and verify changes. `oxispec connect` adds the MCP server to the config of Claude Code, Cursor, Gemini CLI or VS Code, so the agent can search the specs and read them with their links. The agent proposes change sets; a person approves them. See [agents](../agents.md).
