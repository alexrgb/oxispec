---
id: REQ-ENTITY-0008
type: requirement
status: accepted
date: 2026-11-08
---
### Requirement: An attribute with no field in the code is warned about
The system SHALL warn with W025 about an attribute of an entity that no field of the code representing it matches, ignoring case, `_` and `-`, name the entity and the type, report it in `oxispec entities` and in `oxispec ci`, and say nothing when no code that names the entity defines a type with fields.

#### Scenario: A missing field {#S1}
- **WHEN** the struct has `total` and the entity has `total` and `status`
- **THEN** `entities` says there is no field for `order.status`, `ci` reports W025 for `status` and fails only with `--strict`, and naming the field in any case settles it

#### Scenario: Nothing to look in {#S2}
- **WHEN** the code that names the entity is a function, or a type with no fields
- **THEN** nothing is said about the attributes
