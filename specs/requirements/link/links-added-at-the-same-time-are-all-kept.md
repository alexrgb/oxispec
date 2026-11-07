---
id: REQ-LINK-0002
type: requirement
status: accepted
date: 2026-11-07
---
### Requirement: Links added at the same time are all kept
The system SHALL keep every link when several `oxispec link` commands change the same file at the same time.

#### Scenario: Parallel additions {#S1}
- **WHEN** several `link` commands add different links to one node at once
- **THEN** every link is in the file afterwards
