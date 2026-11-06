---
id: REQ-RISK-0002
type: requirement
status: accepted
date: 2026-11-06
---
### Requirement: The level comes from likelihood and impact
The system SHALL work out the level of a risk from its likelihood and impact, so that nobody writes it by hand, and SHALL give no level when either is missing or not valid.

#### Scenario: The matrix {#S1}
- **WHEN** a risk has medium likelihood and critical impact
- **THEN** its level is high, and high likelihood with high impact is critical, and low with low is low
