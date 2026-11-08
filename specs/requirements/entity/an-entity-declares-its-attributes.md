---
id: REQ-ENTITY-0001
type: requirement
status: accepted
date: 2026-11-08
---
### Requirement: An entity declares its attributes
The system SHALL read an entity from the `entity` block of a node of type `entity`, with a name and typed attributes, make each attribute a variable named `ENTITY.ATTRIBUTE` that claims can use, report with E041 an entity that cannot be read, has no name or shares its name, and warn with W024 when a node is `about` something that is not an entity.

#### Scenario: Attributes as variables {#S1}
- **WHEN** an entity `order` has the attributes `total` and `status`, and a requirement claims `order.total <= 100`
- **THEN** the entity is read with its attributes and the claim is checked against the variable `order.total`, with no error

#### Scenario: A bad entity {#S2}
- **WHEN** an entity block has no name, another has an enum with no values, a node has no block, and two entities share a name
- **THEN** each is reported as E041 against its node, and the entity that is fine is not

#### Scenario: About something else {#S3}
- **WHEN** a node is `about` a requirement
- **THEN** `check` warns with W024 against that node
