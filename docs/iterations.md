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
| 5a | `feat/5a-change-sets` | Change sets as nodes: `new change`, the lifecycle (`change status`, `change list`), a recorded approval, checks E012 and E013 | A change set moves along its lifecycle, skipping a step is refused, and an approved one without an approver fails `check`. **In review.** |
| 5b | `feat/5b-releases` | `release prepare`, `check`, `publish`, `list`. Release notes from the change sets and the diff since the last release, a suggested version bump, gates (no draft or in-review change sets, no errors). A release is a spec tag | `release check` fails on a change set still in review. Notes are generated |
| 6 | `feat/6-ci-command` | `oxispec ci`: graph diff, impact set, new-findings-only, SARIF and JUnit reports, pull request comment, GitHub Actions example | A pull request run reports only new findings and uploads SARIF |
| 7 | `feat/7-search-and-site` | tantivy text search, static site renderer with versioned ID links | `oxispec site` builds pages. Links resolve to the same revision |
| 8 | `feat/8-scanners-and-trace` | tree-sitter query based scanners for Go, Rust and Node, anchors, test result import, coverage, suspect links | A changed function marks its link suspect. Coverage report lists uncovered scenarios |
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
- **Iteration 5a** (in review): change sets as nodes, with a lifecycle and a recorded approval. See [format.md](format.md#change-sets).
