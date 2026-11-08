---
id: REQ-ROBUSTNESS-0002
type: requirement
status: accepted
date: 2026-11-08
---
### Requirement: Commands that write at the same time do not collide
The system SHALL give different IDs to nodes made at the same time, and, with the random ID scheme, IDs that do not collide across branches.

#### Scenario: At once {#S1}
- **WHEN** many `new` commands run at the same time
- **THEN** no ID is used twice and every node is there

#### Scenario: Random IDs {#S2}
- **WHEN** the random scheme makes IDs
- **THEN** each has a four-character suffix, two differ, and a flag overrides the project setting
