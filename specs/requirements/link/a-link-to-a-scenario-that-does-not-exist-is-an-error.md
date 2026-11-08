---
id: REQ-LINK-0003
type: requirement
status: accepted
date: 2026-11-08
---
### Requirement: A link to a scenario that does not exist is an error
The system SHALL report with E009 a link to a scenario that the node does not have, and name the scenario.

#### Scenario: A missing scenario {#S1}
- **WHEN** a node links to `REQ-AUTH-0001.S9` and the requirement has no such scenario
- **THEN** `check` exits with 1 and reports E009 with `S9`
