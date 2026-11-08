---
id: REQ-CHANGE-0004
type: requirement
status: accepted
date: 2026-11-08
---
### Requirement: The list of change sets has what a release needs
The system SHALL list change sets as JSON with `schema_version`, and for each its ID, status, title, approver and the nodes it changes.

#### Scenario: The list {#S1}
- **WHEN** `change list --json` is run with an approved change set
- **THEN** the entry has the ID, `approved`, the title, the approver and the changed nodes
