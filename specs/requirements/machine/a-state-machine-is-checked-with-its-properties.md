---
id: REQ-MACHINE-0001
type: requirement
status: accepted
date: 2026-11-07
---
### Requirement: A state machine is checked with its properties
The system SHALL check a state machine for a valid structure and check each property of it, and SHALL show the path of states and events that breaks a property.

#### Scenario: A broken property {#S1}
- **WHEN** a new machine and then a machine with a property that does not hold
- **THEN** the new one passes `check`, the other fails with the codes E023 and E024 and the path that breaks the property
