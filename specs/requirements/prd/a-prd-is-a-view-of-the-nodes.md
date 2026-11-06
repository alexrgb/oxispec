---
id: REQ-PRD-0001
type: requirement
status: accepted
date: 2026-11-06
---
### Requirement: A PRD is made from the nodes
The system SHALL write the product requirements document of a `prd` node from that node's sections and the requirements that refine it, with each scenario as an acceptance criterion, and SHALL refuse any other node.

#### Scenario: The document {#S1}
- **WHEN** a `prd` node has sections and requirements that refine it
- **THEN** the document holds the sections, then each requirement with its scenarios, and a section with no text says it is not written yet

#### Scenario: Another node {#S2}
- **WHEN** the node is not a `prd`, or does not exist
- **THEN** the document is refused with the reason
