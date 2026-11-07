---
id: REQ-ARCHITECTURE-0001
type: requirement
status: accepted
date: 2026-11-07
---
### Requirement: Architecture elements keep their structure
The system SHALL create the elements of the architecture model as nodes, and report an error for a container that has no parent system.

#### Scenario: Elements {#S1}
- **WHEN** a system and an environment are created, and then a container with no parent
- **THEN** the system gets the ID C4-0001 and the environment DEP-0001, `check` passes, and the container fails `check`
