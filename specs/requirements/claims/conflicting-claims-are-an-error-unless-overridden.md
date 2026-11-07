---
id: REQ-CLAIMS-0001
type: requirement
status: accepted
date: 2026-11-07
---
### Requirement: Conflicting claims are an error unless one overrides
The system SHALL report an error naming both requirements and the condition when two requirements make claims about the same thing that cannot both hold, and SHALL accept the conflict when the narrower requirement has an `overrides` link to the wider one.

#### Scenario: A conflict {#S1}
- **WHEN** two requirements disagree for the same role
- **THEN** `check` fails with E019, names the condition and both IDs, and after an `overrides` link `check` passes
