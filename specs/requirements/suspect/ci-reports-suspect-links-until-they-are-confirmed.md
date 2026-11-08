---
id: REQ-SUSPECT-0002
type: requirement
status: accepted
date: 2026-11-08
---
### Requirement: CI reports suspect links until they are confirmed
The system SHALL report with `ci` as a new error (E016) a link whose code changed since it was confirmed, with the file of the code, and SHALL stop reporting it once `confirm` records that code and spec agree.

#### Scenario: A changed function {#S1}
- **WHEN** a function is confirmed and then changed, and `ci` is run
- **THEN** `ci` exits with 1, reports E016 with the path of the code, and after `confirm` it exits with 0
