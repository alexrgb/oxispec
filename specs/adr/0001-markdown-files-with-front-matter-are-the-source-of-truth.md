---
id: ADR-0001
type: adr
status: accepted
date: 2026-11-05
---
# ADR-0001: Markdown files with front matter are the source of truth

## Context

Specs, decisions and change sets must be reviewed in pull requests, have a history, and be readable by people and by agents. A database would need its own history, review and access rules, and would keep a second copy of what git already holds.

## Decision

Every node is a markdown file with YAML front matter, kept in the project's git repository. Git is the authority. Everything else (the search index, the cache, the site) is derived from the files and can be rebuilt from them.

## Consequences

History, diffs, blame, branches and rollback are git's. A node can be edited in any editor, and by an agent. The cost is that cross-node questions need a parse of all files, which `oxispec index` keeps cheap with a local cache that is never committed.
