---
id: REQ-MACHINE-0002
type: requirement
status: accepted
date: 2026-11-07
---
### Requirement: A state machine is shown, drawn and turned into tests
The system SHALL list state machines, show one with its transitions and whether each property holds, draw it as a Mermaid diagram, and write test cases from it that name the requirements they implement.

#### Scenario: The views {#S1}
- **WHEN** a machine with one property that holds and one that does not
- **THEN** the list, the detail, the JSON, the diagram and the generated test cases all agree on the transitions and properties
