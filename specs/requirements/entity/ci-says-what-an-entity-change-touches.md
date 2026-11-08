---
id: REQ-ENTITY-0005
type: requirement
status: accepted
date: 2026-11-08
---
### Requirement: CI says what an entity change touches
The system SHALL report with `oxispec ci`, as text, JSON and in the pull request comment, each entity that a change alters, what changed in it, whether that can break a reader, and how many nodes depend on it, as information and not as a finding, and SHALL report nothing for an entity the change does not touch.

#### Scenario: A change {#S1}
- **WHEN** an entity gains a value and loses an attribute that a claim uses
- **THEN** the report names the entity, lists `values-added` as compatible and `attribute-removed` as breaking, names the nodes that depend on it, and the run fails only because of the error in the claim; before the change the entities list is empty
