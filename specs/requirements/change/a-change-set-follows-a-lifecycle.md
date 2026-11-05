---
id: REQ-CHANGE-0001
type: requirement
status: accepted
date: 2026-11-05
---
### Requirement: A change set follows a lifecycle
The system SHALL let a change set move only along draft, in-review, approved, merged and reverted, and SHALL report a change set whose status is missing or not in the lifecycle.

#### Scenario: The steps allowed {#S1}
- **WHEN** a change set is draft
- **THEN** it can only go to in-review, and an approved one can go back to in-review or on to merged

#### Scenario: A status outside the lifecycle {#S2}
- **WHEN** a change set has no status, or the status `finished`
- **THEN** the finding E012 is reported
