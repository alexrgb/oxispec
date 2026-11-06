---
id: REQ-COMPLIANCE-0002
type: requirement
status: accepted
date: 2026-11-06
---
### Requirement: A requirement past draft meets the rules of the standard
The system SHALL report an accepted requirement that lacks the test or the sign-off the chosen standard and level ask for, SHALL accept one that has them, and SHALL leave a draft alone.

#### Scenario: Missing evidence {#S1}
- **WHEN** an accepted requirement has no test and no sign-off under IEC 62304 class A
- **THEN** the findings E035 and E036 are reported

#### Scenario: Evidence {#S2}
- **WHEN** it has a test on its scenario and a named approver
- **THEN** nothing is reported

#### Scenario: A draft {#S3}
- **WHEN** the requirement is a draft
- **THEN** nothing is reported
