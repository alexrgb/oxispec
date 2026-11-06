---
id: REQ-DEBT-0004
type: requirement
status: accepted
date: 2026-11-06
---
### Requirement: Debt limits count live items
The system SHALL count only live debt items against the limits set per severity, and SHALL report each severity that is over its limit.

#### Scenario: Over a limit {#S1}
- **WHEN** a limit of 1 high and 0 critical is set and the project has 2 live high and 1 live critical items, plus a resolved one
- **THEN** both severities are reported with their counts and limits, and the resolved item is not counted
