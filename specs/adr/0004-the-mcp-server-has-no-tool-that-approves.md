---
id: ADR-0004
type: adr
status: accepted
date: 2026-11-05
---
# ADR-0004: The MCP server has no tool that approves

## Context

An agent that can read the specs and propose changes can also be wrong or be misled by text it read. If it could also approve its own proposal, nothing would stand between a mistake and the merged specs.

## Decision

The server offers `spec_propose_change`, which writes a change set in the `draft` status, and no tool that moves a change set to `approved` or `merged`. The instructions the server sends say that a person approves. A read-only server leaves out the tool that writes.

## Consequences

Approval stays a human step, recorded in git with the commit that makes it. An agent that needs a change approved asks a person. The approval is a record in a file, so repository review rules, not the server, are what stop a person approving their own change.
