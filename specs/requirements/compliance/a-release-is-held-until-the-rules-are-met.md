---
id: REQ-COMPLIANCE-0004
type: requirement
status: accepted
date: 2026-11-08
---
### Requirement: A release is held until the compliance rules are met
The system SHALL list the gate `compliance-met` for a release in a project with a standard, and hold the release back while a rule is not met.

#### Scenario: The gate {#S1}
- **WHEN** a release is prepared in a project that chose a standard
- **THEN** the gate is listed, and the release is held while a rule is missing
