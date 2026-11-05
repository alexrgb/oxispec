---
id: REQ-FORMAT-0006
type: requirement
status: accepted
date: 2026-11-05
links:
  decided-by:
    - ADR-0003
---
### Requirement: Reverse links are computed
The system SHALL list the nodes that link to a node without any of them being written in that node's file.

#### Scenario: What links here {#S1}
- **WHEN** a requirement has a `decided-by` link to ADR-0001 and nobody wrote anything in ADR-0001's file
- **THEN** asking for the nodes that link to ADR-0001 returns the requirement with the kind `decided-by`
