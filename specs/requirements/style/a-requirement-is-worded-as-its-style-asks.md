---
id: REQ-STYLE-0001
type: requirement
status: accepted
date: 2026-11-06
---
### Requirement: A requirement is worded as its style asks
The system SHALL tell the six EARS patterns apart and say what is wrong with a statement that fits none, SHALL require a role, a goal and a benefit of a user story, and SHALL require a WHEN and a THEN of a Gherkin scenario.

#### Scenario: EARS {#S1}
- **WHEN** statements of the six EARS patterns, with MUST and any case
- **THEN** each is recognised, and a statement that fits none is told what is wrong

#### Scenario: User story {#S2}
- **WHEN** a statement that lacks the role, the goal or the benefit
- **THEN** the missing part is named

#### Scenario: Gherkin {#S3}
- **WHEN** a scenario without a WHEN or without a THEN
- **THEN** the missing step is named
