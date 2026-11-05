---
id: ADR-0002
type: adr
status: accepted
date: 2026-11-05
---
# ADR-0002: IDs are immutable and independent of file paths

## Context

Links, tests and code annotations refer to a node for years. If a link were a path, renaming or moving a file would break it, and if an ID were reused, an old reference would silently mean something else.

## Decision

A node has an `id` in its front matter. An ID never changes and is never reused. It does not depend on where the file lives, so moving or renaming the file keeps every link. A scenario is addressed as a child of its requirement, `REQ-AUTH-0012.S1`, by an anchor in its heading.

## Consequences

`oxispec check` has to detect duplicate IDs (E001) because two files may claim one. Deleting a node leaves its ID retired. Branches that add nodes at the same time can choose the same sequential number, so a random suffix scheme is offered.
