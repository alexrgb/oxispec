---
id: REQ-FORMAT-0007
type: requirement
status: accepted
date: 2026-11-12
---
### Requirement: A node can answer to another name
The system SHALL let a node list other names in `aliases`, SHALL read a link, a code marker or a command that uses an alias as one that uses the node's ID, and SHALL report an error when an alias is not a valid ID, is the ID of another node, or belongs to two nodes.

#### Scenario: An alias in a link and in code {#S1}
- **WHEN** a node has `aliases: [CAR-1]`, another node links to `CAR-1`, and a test says `verifies: CAR-1.S1`
- **THEN** `check` finds nothing wrong, `show CAR-1` shows the node, and the node is verified

#### Scenario: A name that is taken {#S2}
- **WHEN** an alias is the ID of another node, or two nodes list the same alias
- **THEN** the finding E042 is reported
