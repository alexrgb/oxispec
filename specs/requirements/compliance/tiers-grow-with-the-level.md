---
id: REQ-COMPLIANCE-0001
type: requirement
status: accepted
date: 2026-11-06
---
### Requirement: Rules grow with the level
The system SHALL give a compliance level more rules than the level below it, so that a stricter level asks for more evidence.

#### Scenario: Tiers {#S1}
- **WHEN** the tiers 0 to 3 are asked for their rules
- **THEN** tier 0 has none and each higher tier has more than the one below
