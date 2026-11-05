---
id: REQ-FORMAT-0004
type: requirement
status: accepted
date: 2026-11-05
---
### Requirement: A requirement has a SHALL and a scenario
The system SHALL report an error for a `### Requirement:` block that has no `#### Scenario:` or whose description has no SHALL or MUST in capitals, and for a requirement node that has no requirement block.

#### Scenario: No scenario and no SHALL {#S1}
- **WHEN** a requirement block says "It works." and has no scenario
- **THEN** the findings E006 and E007 are reported

#### Scenario: No block {#S2}
- **WHEN** a node of type requirement has no `### Requirement:` heading
- **THEN** the finding E008 is reported
