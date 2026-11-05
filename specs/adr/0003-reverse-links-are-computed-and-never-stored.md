---
id: ADR-0003
type: adr
status: accepted
date: 2026-11-05
---
# ADR-0003: Reverse links are computed and never stored

## Context

A link between two nodes could be written on both ends so that each side shows it. Two copies of one fact drift apart, and every edit has to touch two files.

## Decision

A link is written once, in the front matter of the node it starts from. The other direction is computed from all the links when it is asked for, as in `oxispec show` and `spec_get`.

## Consequences

Adding a link changes one file. A reader never sees a half-written pair. Finding what points at a node needs the whole graph, which the loader already builds.
