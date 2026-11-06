---
id: REQ-DEBT-0003
type: requirement
status: accepted
date: 2026-11-06
---
### Requirement: The debt backlog is ordered
The system SHALL list the debt that is still to be paid with the most serious first and, within a severity, the oldest first.

#### Scenario: Order {#S1}
- **WHEN** the backlog has items of several severities and dates
- **THEN** the most serious come first and, within a severity, the oldest
