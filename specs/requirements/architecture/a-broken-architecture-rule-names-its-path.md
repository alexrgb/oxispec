---
id: REQ-ARCHITECTURE-0003
type: requirement
status: accepted
date: 2026-11-07
---
### Requirement: A broken architecture rule names its path
The system SHALL report a rule of the model that does not hold with the path that breaks it, and CI SHALL report it once as a new finding and as an existing one after that.

#### Scenario: A forbidden path {#S1}
- **WHEN** a rule forbids a path and a branch adds the path
- **THEN** `ci` exits with 1 and reports E027 as new, and on a later change to the branch it counts the finding as existing
