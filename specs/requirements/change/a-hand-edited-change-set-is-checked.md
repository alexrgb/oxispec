---
id: REQ-CHANGE-0003
type: requirement
status: accepted
date: 2026-11-08
---
### Requirement: A hand-edited change set is checked
The system SHALL check change sets that were edited by hand and report the codes E012 and E013 for what is not valid in them.

#### Scenario: Bad change sets {#S1}
- **WHEN** a change set is edited so that it is not valid
- **THEN** `check` exits with 1 and reports E012 or E013
