---
id: REQ-MCP-0008
type: requirement
status: accepted
date: 2026-11-08
---
### Requirement: A running server sees changes to the files
The system SHALL answer a running MCP server's calls from the files as they are after an edit, a new file or a change to the format profiles, without a restart.

#### Scenario: Edits {#S1}
- **WHEN** a node is edited and a new file is added while the server runs
- **THEN** the next calls see the new title and the new node

#### Scenario: Format profiles {#S2}
- **WHEN** the config of the profiles is changed while the server runs
- **THEN** `spec_check` applies the new profile
