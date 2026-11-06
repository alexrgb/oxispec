---
id: REQ-RISK-0001
type: requirement
status: accepted
date: 2026-11-06
---
### Requirement: A risk has valid fields
The system SHALL report a risk whose status, likelihood or impact is missing or not one of the allowed values, or whose date or review day is not a date, and SHALL raise nothing for a complete risk.

#### Scenario: Fields {#S1}
- **WHEN** a risk has the status `later`, the likelihood `certain`, the impact `huge` and dates that are not dates
- **THEN** the finding E038 is reported for each, and a complete risk raises nothing
