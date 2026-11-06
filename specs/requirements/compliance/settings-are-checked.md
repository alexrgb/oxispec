---
id: REQ-COMPLIANCE-0003
type: requirement
status: accepted
date: 2026-11-06
---
### Requirement: The compliance settings are checked
The system SHALL report a compliance setting that names a standard or level that does not exist, and SHALL check the project's own rules, reporting a missing link and a rule that asks for nothing.

#### Scenario: Bad settings {#S1}
- **WHEN** the config names the standard `nope` or the level `Z`
- **THEN** the finding E037 is reported

#### Scenario: Own rules {#S2}
- **WHEN** a rule asks for a `decided-by` link and a requirement has none, or a rule asks for nothing
- **THEN** E035 is reported for the first and E037 for the second
