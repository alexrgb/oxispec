# The project's own specs

OxiSpec describes itself with OxiSpec. The `specs/` folder holds nodes for the parts of the product that are written down as specs, and `oxispec check` and `oxispec coverage` run on them in CI (the `specs` job in `.github/workflows/ci.yml`). It covers the rules of the node file format, change sets, the MCP server, PRDs, API operations, tech debt, releases, risks, test plans, compliance profiles, format profiles and styles, import, search, diffs, rollback, impact, context packs, the trace scan and suspect links. The rest of the documentation is still plain markdown in `docs/`.

## What is there

| Node | About |
|---|---|
| `REQ-FORMAT-0001` | A node is a file with an `id` and a `type` |
| `REQ-FORMAT-0002` | Node IDs are unique |
| `REQ-FORMAT-0003` | Links point at things that exist |
| `REQ-FORMAT-0004` | A requirement has a SHALL and a scenario |
| `REQ-FORMAT-0005` | Scenario anchors are unique and valid |
| `REQ-FORMAT-0006` | Reverse links are computed |
| `REQ-CHANGE-0001`, `0002` | A change set follows a lifecycle; approval is recorded and can be withdrawn |
| `REQ-MCP-0001` to `0004` | The read tools answer as the commands do; tools are offered by module and mode; arguments cannot be read as options; an agent can propose a change set but never approve it |
| `REQ-PRD-0001` | A PRD is made from the nodes |
| `REQ-API-0001` to `0006` | Operations come from the requirements; a description is compared with them; an operation has a card; removing from the API is breaking; a running service can be compared; an operation says what happens to a caller who may not use it |
| `REQ-TRACE-0001` to `0003` | Annotations are read from comments; coverage is counted from annotations; a marker inside a string is not an annotation |
| `REQ-SUSPECT-0001` | A link goes suspect when either side changes |
| `REQ-SEARCH-0001`, `0002` | Search finds what is written; the index follows the files |
| `REQ-DIFF-0001`, `0002` | A diff names each kind of change; graphs are compared by node |
| `REQ-ROLLBACK-0001`, `0002` | A rollback names an action for each node; it is checked before it is made |
| `REQ-IMPACT-0001` | A change brings in the nodes that depend on it |
| `REQ-CONTEXT-0001`, `0002` | Neighbors follow links both ways; a context pack fits a budget |
| `REQ-COMPLIANCE-0001` to `0003` | Rules grow with the level; a requirement past draft meets the rules of the standard; the compliance settings are checked |
| `REQ-STYLE-0001`, `0002` | A requirement is worded as its style asks; a node meets its format profile |
| `REQ-IMPORT-0001`, `0002` | An import reads the specs of other tools; a plan proposes what each file becomes |
| `REQ-TESTPLAN-0001`, `0002` | A test plan is checked; a plan is read against the tests |
| `REQ-RISK-0001` to `0005` | A risk has valid fields; the level is worked out; a risk is accepted by a person; the register is ordered; a critical risk holds a release back |
| `REQ-DEBT-0001` to `0004` | A debt item has valid fields; a live item is held to account; the backlog is ordered; limits count live items |
| `REQ-RELEASE-0001` to `0003` | The suggested version follows SemVer; gates hold a release back; release notes list what changed |
| `ADR-0001` to `ADR-0004` | Why files are the source of truth, why IDs do not depend on paths, why reverse links are not stored, why the MCP server has no tool that approves |

Each scenario is verified by an existing unit test, which carries a `// verifies: REQ-FORMAT-000N.SM` comment, and the code that does the work carries `// spec: REQ-FORMAT-000N`. `oxispec trace REQ-FORMAT-0004` shows the ties.

The reference for the format is still [format.md](format.md). A requirement here says what must hold; format.md explains it for a user.

## Changing the format

When a change alters one of these rules, change the requirement in the same pull request, and keep the test that verifies it. CI fails if a requirement loses its test (`coverage --fail-under 100`) or an annotation names a node that does not exist (E014). To add a rule, write the requirement with `oxispec new requirement format "..."`, write the scenarios, and tie a test to each.

## The trace scan and test data

The scan reads comments without parsing the language, but it does not read a marker that stands inside a double-quoted string on its line, so `"// spec: REQ-A-0001"` in a test is text and not an annotation. A string that runs over several lines, and an example in a doc comment, are still read.

`.oxispec/config.yaml` leaves a few files out of the scan (`trace.exclude`): the tests of the scanner itself hold annotations as sample text, in strings that look like comments, and the scan reads comments without parsing the language. If you add such a test, add its file there. A test file that also carries real `verifies:` comments (`crates/oxispec-cli/tests/cli.rs`) is scanned instead, and its few fixtures are written with `concat!("// spe", "c: ...")` so the scan does not see them.

## Publishing

`oxispec site --out public` builds a page for each of these nodes, with what links to each. Publishing it needs a host and a decision about the repository's visibility, which is not made yet.
