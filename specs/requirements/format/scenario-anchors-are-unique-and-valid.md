---
id: REQ-FORMAT-0005
type: requirement
status: accepted
date: 2026-11-05
---
### Requirement: Scenario anchors are unique and valid
The system SHALL report an error when a scenario anchor is used twice in one file, and when an anchor is not made of letters and digits.

#### Scenario: A repeated anchor {#S1}
- **WHEN** two scenarios in one file end with `{#S1}`
- **THEN** the finding E010 is reported

#### Scenario: An anchor with a hyphen {#S2}
- **WHEN** a scenario ends with `{#bad-id}`
- **THEN** the finding E011 is reported
