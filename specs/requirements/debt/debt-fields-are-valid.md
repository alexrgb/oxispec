---
id: REQ-DEBT-0001
type: requirement
status: accepted
date: 2026-11-06
---
### Requirement: A debt item has valid fields
The system SHALL report a debt item whose status or severity is missing or not one of the allowed values, or whose date or due day is not a date, and SHALL raise nothing for a complete item.

#### Scenario: Invalid fields {#S1}
- **WHEN** a debt item has the status `later`, the severity `huge` and dates that are not dates
- **THEN** the finding E032 is reported for each

#### Scenario: A complete item {#S2}
- **WHEN** a debt item has a valid status, severity, owner, source and dates
- **THEN** no finding is reported
