---
id: REQ-CONTEXT-0001
type: requirement
status: accepted
date: 2026-11-06
---
### Requirement: Neighbors follow links both ways
The system SHALL find the nodes near a node by following links in both directions, limited by depth, link kinds and a count.

#### Scenario: Both directions {#S1}
- **WHEN** a node links to one and is linked from another
- **THEN** both are neighbors

#### Scenario: Limits {#S2}
- **WHEN** depth, kinds or a limit are given
- **THEN** the neighbors respect each
