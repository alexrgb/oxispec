---
id: REQ-ENTITY-0002
type: requirement
status: accepted
date: 2026-11-08
---
### Requirement: What depends on an entity is found
The system SHALL find the nodes that depend on an entity: those that link to it, those whose claims use one of its attributes, and, up to a depth, the nodes that link to those, nearest first, and say how each is connected, and SHALL list the entities that differ between two graphs with what depends on each, leaving out new and unchanged ones.

#### Scenario: Nearest first {#S1}
- **WHEN** a requirement is about an entity, another claims one of its attributes, and a third refines the first
- **THEN** the first two are at distance 1 and say `about ENT-0001` and `claims order.total`, the third is at distance 2, and with a depth of 1 it is left out

#### Scenario: Between two graphs {#S2}
- **WHEN** an entity is changed, one is removed, one is added and one is not touched
- **THEN** the changed and the removed are listed, with what depends on them, and the added and the untouched are not
