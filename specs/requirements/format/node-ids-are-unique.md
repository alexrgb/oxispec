---
id: REQ-FORMAT-0002
type: requirement
status: accepted
date: 2026-11-05
links:
  decided-by:
    - ADR-0002
---
### Requirement: Node IDs are unique
The system SHALL report an error when two files declare the same node ID.

#### Scenario: Two files claim one ID {#S1}
- **WHEN** two files have the front matter `id: ADR-0001`
- **THEN** the finding E001 is reported
