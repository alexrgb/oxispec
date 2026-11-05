---
id: REQ-MCP-0002
type: requirement
status: accepted
date: 2026-11-05
---
### Requirement: Tools are offered by module and by mode
The system SHALL offer a tool only when the module it needs is on, and SHALL leave out the tool that writes when the server runs read-only.

#### Scenario: A module is off {#S1}
- **WHEN** the project has no `tests` module
- **THEN** the tool list has no `spec_trace`

#### Scenario: Read-only {#S2}
- **WHEN** the server starts with `--read-only`
- **THEN** the tool list has no `spec_propose_change` and the read tools remain
