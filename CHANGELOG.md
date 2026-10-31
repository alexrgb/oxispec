# Changelog

All notable changes to this project are recorded here. The format follows [Keep a Changelog](https://keepachangelog.com), and the project follows [Semantic Versioning](https://semver.org). See [docs/versioning.md](docs/versioning.md) for the rules. Only release pull requests edit this file.

## [Unreleased]

## [0.1.0] - 2026-10-31

First release.

### Added

- Nodes as markdown with YAML frontmatter, a linked graph with computed reverse links, and structural checks (`init`, `modules`, `new`, `link`, `check`, `show`). Modules and presets choose what a project uses.
- A local index with content hashing, and a text index for `search`.
- History from git: `--at` on `show`, `check` and other commands, `history`, `diff`, `rollback` and `tag`.
- Change sets with a lifecycle and a recorded approval, and `release prepare`, `check`, `publish` and `list` with generated notes and gates.
- `oxispec ci`: only the findings a change introduces, with SARIF, JUnit and pull request comment output.
- `oxispec site`: a static site with a page per node.
- Trace and coverage from `spec:` and `verifies:` comments, suspect links bound with tree-sitter, and test result import (JUnit XML, `go test -json`).
- Claims and a vocabulary with an exact solver that reports conflicts with the smallest set of claims.
- State machines with property checks, counterexample paths, Mermaid diagrams and generated tests.
- An architecture model (C4 and deployment) with Mermaid views and `forbid` rules.
- Skills for Claude Code, Cursor, GitHub Copilot and Gemini CLI, and `agents export`.
- An MCP server (`oxispec mcp`) with context packs, and `oxispec connect` to write its entry into each client's config.
- `oxispec status`, a summary of what is in the project and what needs attention.
- `oxispec-stress`, a development tool for stress and load tests.

### Fixed

- Concurrent searches, `new` and `link` no longer corrupt the index, repeat an ID or lose an edit.
- A byte order mark in a spec file is read, and a closed pipe no longer panics.
