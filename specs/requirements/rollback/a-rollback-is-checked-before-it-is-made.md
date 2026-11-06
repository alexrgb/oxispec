---
id: REQ-ROLLBACK-0002
type: requirement
status: accepted
date: 2026-11-06
---
### Requirement: A rollback is checked before it is made
The system SHALL work out the graph a rollback would give, and report the errors it would add, such as removing a node that others link to, and none when it changes nothing.

#### Scenario: The graph it would give {#S1}
- **WHEN** restored nodes are swapped in and removed ones dropped
- **THEN** the graph shows exactly that

#### Scenario: A new error {#S2}
- **WHEN** a node that others link to would be removed
- **THEN** a new error is reported

#### Scenario: No change {#S3}
- **WHEN** a rollback changes nothing
- **THEN** it adds no errors
