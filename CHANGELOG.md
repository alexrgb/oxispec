# Changelog

All notable changes to this project are recorded here. The format follows [Keep a Changelog](https://keepachangelog.com), and the project follows [Semantic Versioning](https://semver.org). See [docs/versioning.md](docs/versioning.md) for the rules. Only release pull requests edit this file.

## [Unreleased]

## [0.1.0] - 2026-11-08

First release. The program is early: the file format, the command line and the JSON outputs may still change before 1.0 (see [docs/versioning.md](docs/versioning.md)), and the library `oxispec-core` has no stability promise yet.

### Added

- **The graph.** Nodes as markdown with YAML front matter, typed links with computed reverse links, scenario IDs, and structural checks (`init`, `modules`, `new`, `link`, `check`, `show`). Modules and presets choose what a project uses. A local index with content hashing, and a text index for `search`.
- **History.** `--at` on `show`, `check` and other commands, `history`, `diff`, `rollback`, `tag`, and `check --changed`.
- **Change and release.** Change sets with a lifecycle and a recorded approval, and `release prepare`, `check`, `publish` and `list` with generated notes, a suggested SemVer bump and gates.
- **CI.** `oxispec ci` reports only the findings a change introduces, with SARIF, JUnit and pull request comment output. `oxispec site` builds a static site, with a version picker for releases.
- **Trace.** `spec:` and `verifies:` comments in any language, coverage, suspect links bound with tree-sitter for Rust, Go, TypeScript and JavaScript, `confirm`, test result import (JUnit XML, `go test -json`), and a setting for the languages that are scanned.
- **Consistency.** Claims and a vocabulary with an exact solver that reports conflicts with the smallest set of claims. State machines with property checks, counterexample paths, Mermaid diagrams and generated tests. An architecture model (C4 and deployment) with Mermaid views and `forbid` rules.
- **Entities.** A node type for the things the specs are about, with typed attributes that claims can use. `oxispec entities` shows what depends on one, `ci` says what a change to one touches and whether it can break a reader, the code that represents it is linked with a `spec:` comment and checked for a field for each attribute, and the nodes about an entity are to review when it changes.
- **Records of work.** Tech debt, a risk register, test plans read against the tests, and product requirements documents, each a node type with checks, a command and a release gate where it fits.
- **Formats and import.** Format profiles, ADR formats (MADR, Nygard, Y-statement) with `adr convert` and `import adr`, requirement styles (EARS, user stories, Gherkin), compliance profiles, and `import specs` from OpenSpec, Spec Kit, Kiro, plain markdown, a PRD and an OpenAPI description, with a plan that can be looked at and changed first.
- **APIs.** Requirements titled as calls are the operations of an API. `docs openapi` writes an OpenAPI skeleton and compares a file or a running service with the requirements, `api show` gives the card of an operation, and `api index` and `api search` find an operation by meaning with a built-in embedder or an embedding service on your machine.
- **Agents.** Skills for Claude Code, Cursor, GitHub Copilot and Gemini CLI, with `agents update`, `check` and `export`. An MCP server (`oxispec mcp`) with tools, resources, prompts and context packs, `oxispec serve` for many clients over HTTP with limited tokens and an audit log, and `oxispec connect` to write the server into each client's config.
- **Documentation.** A user guide, a command reference, a table of check codes and an MCP tools reference written from the program, and runnable examples of the library.
- `oxispec status`, `oxispec token`, and `oxispec-stress` (a development tool for stress and load tests, not published).

### Fixed

- Concurrent searches, `new` and `link` no longer corrupt the index, repeat an ID or lose an edit.
- A byte order mark in a spec file is read, and a closed pipe no longer panics.
- `machine --tests` no longer runs out of memory on a large machine, and large projects check in seconds (see [docs/review.md](docs/review.md)).

### Known limits

- No hosted server, no sign-in (OIDC) and no database: the specs are files in git.
- The embedding service for `api search` must be reached over plain `http://`, and the built-in embedder matches words, not meaning.
- Claims and entities describe numbers, flags and choices, not free text. There is no check of requirements written in natural language.
- Entities and their code are compared by name, not by type.
- Releases are built for macOS (Apple silicon and Intel), Linux (x86_64 and arm64, glibc) and Windows (x86_64). The binaries are not signed.
