---
id: REQ-TESTPLAN-0001
type: requirement
status: accepted
date: 2026-11-06
---
### Requirement: A test plan is checked
The system SHALL report a test plan whose status or date is not valid, warn about a plan that covers nothing, and warn about an accepted requirement or a live high-level risk that no approved plan covers, but only in a project that has an approved plan.

#### Scenario: A plan {#S1}
- **WHEN** a test plan has the status `someday` and no `covers` link
- **THEN** the finding E040 and the warning W020 are reported, and a valid plan that covers a requirement raises nothing

#### Scenario: Gaps {#S2}
- **WHEN** a project has an approved plan, an accepted requirement and a live high risk that it does not cover
- **THEN** W021 and W022 are reported, and a project with only a draft plan sees neither
