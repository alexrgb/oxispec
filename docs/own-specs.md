# The project's own specs

OxiSpec describes itself with OxiSpec. The `specs/` folder holds nodes for the parts of the product that are written down as specs, and `oxispec check` and `oxispec coverage` run on them in CI (the `specs` job in `.github/workflows/ci.yml`). It is a small start: the rules of the node file format. The rest of the documentation is still plain markdown in `docs/`.

## What is there

| Node | About |
|---|---|
| `REQ-FORMAT-0001` | A node is a file with an `id` and a `type` |
| `REQ-FORMAT-0002` | Node IDs are unique |
| `REQ-FORMAT-0003` | Links point at things that exist |
| `REQ-FORMAT-0004` | A requirement has a SHALL and a scenario |
| `REQ-FORMAT-0005` | Scenario anchors are unique and valid |
| `REQ-FORMAT-0006` | Reverse links are computed |
| `ADR-0001` to `ADR-0003` | Why files are the source of truth, why IDs do not depend on paths, why reverse links are not stored |

Each scenario is verified by an existing unit test, which carries a `// verifies: REQ-FORMAT-000N.SM` comment, and the code that does the work carries `// spec: REQ-FORMAT-000N`. `oxispec trace REQ-FORMAT-0004` shows the ties.

The reference for the format is still [format.md](format.md). A requirement here says what must hold; format.md explains it for a user.

## Changing the format

When a change alters one of these rules, change the requirement in the same pull request, and keep the test that verifies it. CI fails if a requirement loses its test (`coverage --fail-under 100`) or an annotation names a node that does not exist (E014). To add a rule, write the requirement with `oxispec new requirement format "..."`, write the scenarios, and tie a test to each.

## The trace scan and test data

`.oxispec/config.yaml` leaves a few files out of the scan (`trace.exclude`): the tests of the scanner itself hold annotations as sample text, in strings that look like comments, and the scan reads comments without parsing the language. If you add such a test, add its file there.

## Publishing

`oxispec site --out public` builds a page for each of these nodes, with what links to each. Publishing it needs a host and a decision about the repository's visibility, which is not made yet.
