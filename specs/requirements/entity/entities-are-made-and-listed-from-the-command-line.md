---
id: REQ-ENTITY-0004
type: requirement
status: accepted
date: 2026-11-08
---
### Requirement: Entities are made and listed from the command line
The system SHALL make an entity with `oxispec new entity`, list the entities with their attributes and the number of dependents with `oxispec entities`, show one with everything that depends on it and how each is connected, give the same as JSON, and need the consistency module.

#### Scenario: Making and listing {#S1}
- **WHEN** an entity is made, given attributes, linked to by a requirement and used by a claim
- **THEN** the list shows it with its attributes and the number of dependents, and `check` passes

#### Scenario: One entity {#S2}
- **WHEN** one entity is asked for, as text and as JSON, and one that does not exist
- **THEN** the dependents are shown with their distance, type and how they are connected, the JSON has `schema_version`, and an unknown entity exits with 2; without the consistency module both commands exit with 2
