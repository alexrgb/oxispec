---
id: REQ-MCP-0004
type: requirement
status: accepted
date: 2026-11-05
links:
  decided-by:
    - ADR-0004
---
### Requirement: An agent can propose a change set but never approve it
The system SHALL create a draft change set from `spec_propose_change`, linked to the nodes it changes, SHALL write nothing when any of those nodes does not exist, and SHALL offer no tool that approves.

#### Scenario: A draft {#S1}
- **WHEN** a client proposes "Tighten timeouts" for REQ-AUTH-0001
- **THEN** a change set with status draft and no approver is written, with a `changes` link to REQ-AUTH-0001

#### Scenario: An unknown target {#S2}
- **WHEN** one of the nodes to change does not exist
- **THEN** the call fails and nothing was written

#### Scenario: Read-only {#S3}
- **WHEN** a read-only server is asked to propose a change
- **THEN** the call is refused
