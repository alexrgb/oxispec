---
id: REQ-MCP-0001
type: requirement
status: accepted
date: 2026-11-05
---
### Requirement: The read tools answer as the commands do
The system SHALL answer a read tool call with the same data as the matching command with `--json`, and SHALL return an error the client can read when the node or revision asked for does not exist.

#### Scenario: A node {#S1}
- **WHEN** a client calls `spec_get` for REQ-AUTH-0001
- **THEN** it gets the node with its text, links and what links to it, and for an unknown ID an error result that says there is no node with that ID
