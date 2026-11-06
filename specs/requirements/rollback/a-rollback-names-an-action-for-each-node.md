---
id: REQ-ROLLBACK-0001
type: requirement
status: accepted
date: 2026-11-06
---
### Requirement: A rollback names an action for each node
The system SHALL plan, for each node asked for, restoring it, recreating it, removing it or leaving it as it is, according to how the target version differs from the current one, and SHALL reject a node that does not exist and ignore a repeat.

#### Scenario: Each difference {#S1}
- **WHEN** a node differs, one is missing now, one is new and one is the same
- **THEN** they are restored, recreated, removed and left unchanged

#### Scenario: Unknown and repeated {#S2}
- **WHEN** a node does not exist, or a node is named twice (once as a scenario)
- **THEN** the first is an error and the repeat is ignored
