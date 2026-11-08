---
id: REQ-PROJECT-0001
type: requirement
status: accepted
date: 2026-11-08
---
### Requirement: A project starts and its first nodes pass the checks
The system SHALL create a project with `oxispec init`, create nodes with `oxispec new` that take the next ID of their domain, and pass `oxispec check` on what it created, including a project written before the format number existed.

#### Scenario: The first steps {#S1}
- **WHEN** a project is created with the standard preset and two requirements and a decision are added
- **THEN** the config and the folders exist, the IDs are REQ-AUTH-0001, REQ-AUTH-0002 and ADR-0001, `check` passes and `show` finds them

#### Scenario: An early project {#S2}
- **WHEN** a project has no `format` in its config
- **THEN** `new` and `check` still work
