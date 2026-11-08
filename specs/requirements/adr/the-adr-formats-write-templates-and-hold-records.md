---
id: REQ-ADR-0004
type: requirement
status: accepted
date: 2026-11-08
---
### Requirement: The ADR formats write templates and hold new records
The system SHALL write a new decision as the template of its format (MADR, Nygard or Y-statement), with today's date in the form `change status` writes, and hold the records to the format only when the project chose it.

#### Scenario: Templates {#S1}
- **WHEN** decisions are made in each format
- **THEN** each has the sections of its format and today's date, and with no format chosen nothing holds them to one
