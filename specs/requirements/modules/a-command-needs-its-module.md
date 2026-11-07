---
id: REQ-MODULES-0001
type: requirement
status: accepted
date: 2026-11-07
---
### Requirement: A command needs its module
The system SHALL refuse a command whose module is off, exit with 2 and name the module to add, and SHALL offer the matching MCP tool only when the module is on.

#### Scenario: Commands {#S1}
- **WHEN** the claims, state machine or architecture commands are run in a project without their module
- **THEN** each exits with 2 and names the module, and after `modules add` the command works

#### Scenario: Tools {#S2}
- **WHEN** the MCP tool list is read with and without the module
- **THEN** the tool for claims, machines or architecture is listed only when the module is on
