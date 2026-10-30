# Iterations

OxiSpec is built in small iterations. Each iteration is one branch, one pull request, and leaves `main` working. The design lives in [design.md](design.md); this file is the plan.

## Working agreement

- **One iteration, one branch.** Branch from `main` as `feat/<n>-<slug>`, for example `feat/2-local-index`. Fixes use `fix/<slug>`, docs use `docs/<slug>`, CI and tooling use `ci/<slug>`.
- **No direct pushes to `main`** after the baseline commit. Everything goes through a pull request.
- **Commit and PR titles** follow Conventional Commits. Version numbers and `CHANGELOG.md` change only in release pull requests. See [versioning.md](versioning.md).
- **A pull request needs:** passing CI (format, clippy with warnings denied, tests), tests for new behavior, and an updated design doc if a decision changed.
- **Merge with squash**, so `main` has one commit per iteration or fix. Delete the branch after merging.
- **Keep each iteration small:** about a day of work and a diff a person can review. Split anything larger.
- **Each iteration states "done when"** below. That is the review checklist.

## Plan

| # | Branch | Scope | Done when |
|---|---|---|---|
| 1 | `feat/1-core-foundation` | Node model, frontmatter parser, graph, structural checks, modules and presets, CLI: `init`, `modules`, `new`, `check`, `show`, CI | Tests pass on Linux and macOS. `init`, `new`, `check` round trip works. **Merged** (#1). |
| 2 | `feat/2-links-and-ids` | `oxispec link` command, scenario IDs (`{#S1}`) and child-link checks, random ID scheme option, `format` version in the config and `schema_version` in JSON output ([versioning.md](versioning.md)) | A link to a missing scenario is reported. IDs can be allocated without collisions on parallel branches. **Merged** (#7). |
| 3 | `feat/3-local-index` | Local index under `.oxispec/cache` with content hashing and a size-and-time shortcut, `oxispec index`, `--no-cache`, `check --changed`. The `RevisionStore` and `TextIndex` traits move to iterations 4 and 7, when a second implementation exists | A second `check` on an unchanged project reparses nothing. `--changed` checks only touched nodes and what links to them. **Merged** (#8). |
| 4a | `feat/4a-history-read` | Read history from git: `--at` on `show` and `check`, `history`, `diff`. The SQLite revision store is not needed for this, since git already keeps immutable history (design section 6) | `show --at` returns an old node, `history` follows a renamed file, `diff` reports nodes and new or fixed findings. **Merged** (#9). |
| 4b | `feat/4b-rollback-and-tags` | `rollback` and `tag`. SQLite revision store only if history queries are measured to be slow | `rollback` restores nodes from a revision and refuses when it would break links. `tag` names a revision. **Merged** (#10). |
| 5a | `feat/5a-change-sets` | Change sets as nodes: `new change`, the lifecycle (`change status`, `change list`), a recorded approval, checks E012 and E013 | A change set moves along its lifecycle, skipping a step is refused, and an approved one without an approver fails `check`. **Merged** (#11). |
| 5b | `feat/5b-releases` | `release prepare`, `check`, `publish`, `list`. Release notes from the change sets and the diff since the last release, a suggested version bump, gates (no draft or in-review change sets, no errors). A release is a manifest node plus the tag `specs-v<version>` | `release check` fails on a change set still in review. Notes are generated. **Merged** (#15). |
| 6 | `feat/6-ci-command` | `oxispec ci`: graph diff, impact set, new-findings-only, SARIF and JUnit reports, pull request comment, GitHub Actions example | A pull request run reports only new findings and uploads SARIF. **In review.** Scope as built: `oxispec ci` with text, JSON, SARIF, JUnit and markdown output, and a GitHub Actions example. The comment is posted by the workflow. |
| 7a | `feat/7-search-and-site` | tantivy text search: `oxispec search` with a derived index that updates itself, English stemming, type and status filters, snippets. The `TextIndex` trait waits for a second implementation | A search finds a node by its words, an ID or its title, and only changed files are read again. **Merged** (#17). |
| 7b | `feat/7b-site` | `oxispec site`: a static site with a page per node, links by ID, a version picker and changes since the last release. Raw HTML in specs is not run | `oxispec site` builds pages. Links resolve to the same revision. **In review.** |
| 8a | `feat/8a-trace-and-coverage` | Comment annotations (`spec:`, `verifies:`, `[ID.S1]`) in any language, `oxispec trace` and `oxispec coverage`, check codes E014 and E015, `trace.exclude` in the config. No parsers yet | `coverage` lists requirements nothing implements and scenarios no test checks, and fails on a reference to nothing. **Merged** (#19). |
| 8b | `feat/8b-suspect-links` | tree-sitter symbol binding for Go, Rust, TypeScript and JavaScript, a hash of the code without comments or indentation, a committed lock file, suspect links, `oxispec suspect` and `confirm`, check code E016, suspect links reported by `ci` | A changed function marks its link suspect, and `confirm` clears it. **Merged** (#20). |
| 8c | `feat/8c-test-results` | Test result import (JUnit XML, `go test -json`), `coverage --require-pass`, release gates for suspect links and failing tests, reading the committed code at publish | Coverage says verified and passing; `release publish` fails on a suspect link or a failing test. **In review.** |
| 9 | `feat/9-agent-skills` | `SKILL.md` generation, native writers for Claude Code, Cursor, Copilot and Gemini CLI, `agents export`, published `skills/` folder with a parity test | `oxispec init` writes skills for the four clients. The parity test fails on drift |
| 10 | `feat/10-mcp` | MCP server (read, search, trace, propose), scopes, digest, context packs | An agent can fetch a context pack within a token budget |
| 11 | `feat/11-claims-and-solver` | Claims block, vocabulary, built-in solver, conflict reports with minimal cores | The admin timeout example reports a conflict and names both requirements |
| 12 | `feat/12-state-machines` | State machine DSL, static checks, property checks, generated tests | A forbidden path returns a counterexample |
| 13 | `feat/13-architecture` | C4 and deployment model, views, Mermaid output, architecture rules | Container, component and deployment views render from one model |
| 14+ | later | Server mode, RBAC and ABAC, natural language layer, workspaces and federation | See design sections 10, 14 and 19 |

The plan changes as we learn. Change it in a pull request, like anything else.

## Done so far

- **Iteration 1** (#1): node model and frontmatter parser, graph with computed reverse links, structural checks, module registry and presets, CLI `init`, `modules`, `new`, `check`, `show`.
- **CI hardening** (#5): Linux, macOS and Windows tests, minimum Rust version job, `cargo-deny`, lint with rustdoc.
- **YAML parser** (#6): replaced the unmaintained `unsafe-libyaml` with `serde-saphyr`.
- **Iteration 2** (#7): `oxispec link`, scenario anchors and checks for links to scenarios, random ID scheme, `format` in the config, `schema_version` in JSON output. The format is described in [format.md](format.md).
- **Iteration 3** (#8): the local index with content hashing, `oxispec index`, `--no-cache` and `check --changed`. See [format.md](format.md).
- **Iteration 4a** (#9): `--at` on `show` and `check`, `history` and `diff`, read straight from git. See [history.md](history.md).
- **Iteration 4b** (#10): `rollback` (checked, not committed, with `--dry-run`) and `tag`. See [history.md](history.md).
- **CI** (#14): the minimum Rust version is read from `Cargo.toml`, and a non-blocking beta job.
- **Iteration 5a** (#11): change sets as nodes, with a lifecycle and a recorded approval. See [format.md](format.md#change-sets).
- **Iteration 5b** (#15): `release prepare`, `check`, `publish` and `list`, generated notes, a suggested version and gates. See [format.md](format.md#releases).
- **Iteration 6** (#16): `oxispec ci`, with SARIF, JUnit and a pull request comment. See [ci.md](ci.md).
- **Iteration 7a** (#17): `oxispec search`. See [search.md](search.md).
- **Iteration 7b** (#18): `oxispec site`. See [site.md](site.md).
- **Iteration 8a** (#19): `oxispec trace` and `oxispec coverage`. See [trace.md](trace.md).
- **Iteration 8b** (#20): suspect links with tree-sitter. See [trace.md](trace.md#suspect-links).
- **Iteration 8c** (in review): test results and release gates. See [trace.md](trace.md#test-results).
