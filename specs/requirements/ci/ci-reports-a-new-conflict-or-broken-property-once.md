---
id: REQ-CI-0004
type: requirement
status: accepted
date: 2026-11-08
---
### Requirement: CI reports a new conflict or broken property once
The system SHALL report a conflict of claims (E019) or a broken property of a state machine (E023) as new in the change that adds it, and as existing after it is merged, even when its block moves down the file.

#### Scenario: A conflict {#S1}
- **WHEN** a change adds a conflict of claims, and later it is merged and its block moves
- **THEN** the change fails with E019, and after the merge it passes and counts the finding as existing

#### Scenario: A property {#S2}
- **WHEN** a change adds a machine whose property does not hold, and later it is merged and moves
- **THEN** the change fails with E023, and after the merge it counts the finding as existing
