---
id: REQ-IMPORT-0004
type: requirement
status: accepted
date: 2026-11-08
---
### Requirement: A skipped item is not written and an empty plan is refused
The system SHALL not write an item of an import plan that was skipped, SHALL say which were skipped, and SHALL refuse a plan with nothing to write.

#### Scenario: Skipping {#S1}
- **WHEN** one item of a plan is skipped and then all are
- **THEN** the skipped item is named and not written, and a plan with nothing to write exits with an error that says so
