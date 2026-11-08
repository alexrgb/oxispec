---
id: REQ-CHECKING-0002
type: requirement
status: accepted
date: 2026-11-08
---
### Requirement: JSON output says which schema it has
The system SHALL put `schema_version` in the JSON output of its commands, so that a program can tell when the shape changes.

#### Scenario: A schema version {#S1}
- **WHEN** a command is run with `--json`
- **THEN** the output carries `schema_version`
