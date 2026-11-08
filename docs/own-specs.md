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
| `REQ-MCP-0001` to `0005` | The read tools answer as the commands do; tools are offered by module and mode; arguments cannot be read as options; an agent can propose a change set but never approve it; an agent can search the operations |
| `REQ-PRD-0001` | A PRD is made from the nodes |
| `REQ-APIINDEX-0001` to `0004` | Each operation has a chunk of text; the index follows the operations; a query finds the nearest operations; the index is built and searched from the command line |
| `REQ-API-0001` to `0006` | Operations come from the requirements; a description is compared with them; an operation has a card; removing from the API is breaking; a running service can be compared; an operation says what happens to a caller who may not use it |
| `REQ-LINK-0001`, `0002` | A link is added to the node and nothing else changes; links added at the same time are all kept |
| `REQ-HISTORY-0001`, `0002` | History follows a node through git; history needs a git repository |
| `REQ-SERVE-0001` to `0004` | A token keeps only a hash; the server refuses an unsafe start; every request needs a token and is audited; a limited token sees only its types |
| `REQ-SITE-0001` to `0003` | A site has a page for each node; it can show the specs at a revision; it can hold every release |
| `REQ-RESULTS-0001` | Test results say which scenarios pass |
| `REQ-CLAIMS-0001`, `0002` | Conflicting claims are an error unless one overrides; claims are listed with the vocabulary |
| `REQ-MACHINE-0001`, `0002` | A state machine is checked with its properties; it is shown, drawn and turned into tests |
| `REQ-ARCHITECTURE-0001` to `0003` | Elements keep their structure; views come from one model; a broken rule names its path |
| `REQ-MODULES-0001` | A command needs its module, and so does its MCP tool |
| `REQ-LIBRARY-0001` | The examples of the library run |
| `REQ-PROJECT-0001` to `0003` | A project starts and its first nodes pass the checks; modules follow their dependencies; a command says what to do outside a project |
| `REQ-INDEX-0001` | The local index is reused and never changes an answer |
| `REQ-CHECKING-0001`, `0002` | Check can look only at what a change touches; JSON output says which schema it has |
| `REQ-CI-0001` to `0003` | `ci` reports only what the change introduced; it writes reports for the tools around it; it compares with where the branch started |
| `REQ-AGENTS-0001` to `0003` | Skills are written for the chosen clients; they do not drift and hand edits are kept; they can be exported and name real commands |
| `REQ-CONNECT-0001`, `0002` | `connect` adds the server to the clients' config; it can point at a running server |
| `REQ-STATUS-0001` | `status` summarizes the project |
| `REQ-ADR-0001` to `0003` | A decision can be converted; decisions are imported from other tools; a superseded decision is checked against its successor |
| `REQ-FORMATS-0001` | A format profile shapes a node |
| `REQ-TAGS-0001` | A tag names a revision of the specs |
| `REQ-ROBUSTNESS-0001`, `0002` | No input makes a command panic; commands that write at the same time do not collide |
| `REQ-ROLLBACK-0003`, `0004` | A rollback is refused when it would do harm; it puts nodes back without committing |
| `REQ-REVISIONS-0001` | A revision can be named in many ways |
| `REQ-DOCS-0001` | The reference is written from the program |
| `REQ-ENTITY-0001` to `0006` | An entity declares its attributes; what depends on it is found; a change says what can break; entities are made and listed from the command line; `ci` says what a change touches; an agent can ask |
| `REQ-TRACE-0001` to `0004` | Annotations are read from comments; coverage is counted from annotations; only a real comment is an annotation; the languages of the scan are configured |
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

Nothing is left out of the scan (`trace.exclude` is empty). For Rust, Go, TypeScript and JavaScript the scan reads a marker only in a real comment, found with the same parsers as the suspect links, so a test can hold `"// spec: REQ-A-0001"` in a string, or in a raw or multi-line string, and it is text. An example in the middle of a doc comment is not read either. The scenario ID in brackets as a test name is not read in Rust files, where a test is a function with no title, nor in comments. The setting still exists for a project that wants to leave a path out (generated code, fixtures); see [trace.md](trace.md).

## Publishing

`oxispec site --out public` builds a page for each of these nodes, with what links to each. Publishing it needs a host and a decision about the repository's visibility, which is not made yet.

## Claims in the project's own specs

The project uses its own claims too. The vocabulary `VOC-0001` ([specs/vocabulary](../specs/vocabulary/0001-words-of-the-oxispec-specs.md)) declares a few variables, and 13 requirements carry a `claims` block for values that the specs state in words in several places:

| Variable | Says | Where |
|---|---|---|
| `exit.refusal` | A command that refuses bad input exits with 2 | `REQ-PROJECT-0003`, `REQ-CHECKING-0001`, `REQ-HISTORY-0002`, `REQ-ROLLBACK-0004`, `REQ-CONNECT-0001` |
| `exit.findings` | A check that finds errors exits with 1 | `REQ-LINK-0003`, `REQ-CLAIMS-0003`, `REQ-FORMATS-0002`, `REQ-SUSPECT-0002` |
| `mcp.resources_page_size` | A page of MCP resources has 500 nodes | `REQ-MCP-0007` |
| `warnings.fail` | A warning fails `check` and `ci` only with `--strict` | `REQ-ADR-0003`, `REQ-CI-0001` |
| `serve.starts` | The server does not start without a token, or on an open address without a TLS proxy | `REQ-SERVE-0002` |

The `specs` job in CI runs `oxispec check`, so a requirement that changes one of these values without the others is an error (E019), as it should be: the exit codes are a promise to scripts. A trial run confirmed it: changing one of the `exit.findings` claims to 2 gave `E019` naming both requirements, and restoring it gave a clean check.

Only 13 of the 126 requirements have claims. A claim says one thing about one variable, so it fits a number, a flag or a choice that is repeated, and not most of what the requirements say. They are added where a value is stated twice.
