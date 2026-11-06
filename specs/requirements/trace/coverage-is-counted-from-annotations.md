---
id: REQ-TRACE-0002
type: requirement
status: accepted
date: 2026-11-06
---
### Requirement: Coverage is counted from annotations
The system SHALL count the requirements that code implements and the scenarios that a test verifies, list the scenarios no test names, and report a reference to a node or scenario that does not exist as an error with its place.

#### Scenario: Counts {#S1}
- **WHEN** one of two scenarios is verified and the requirement is implemented
- **THEN** one requirement is implemented, one of two scenarios is verified and the other is listed

#### Scenario: References to nothing {#S2}
- **WHEN** code refers to a node that does not exist, and to a scenario that does not exist
- **THEN** E014 and E015 are reported with the line
