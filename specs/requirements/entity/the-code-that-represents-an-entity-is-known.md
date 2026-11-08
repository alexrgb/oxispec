---
id: REQ-ENTITY-0007
type: requirement
status: accepted
date: 2026-11-08
---
### Requirement: The code that represents an entity is known
The system SHALL take as the code that represents an entity the code a `spec: ENT-...` comment is above, show it with `oxispec entities` (its place, its name and kind, and the fields of a type), name it in the report of `oxispec ci` when the entity changes, and treat the link like any link between a spec and code: suspect when either side changes, until `confirm`.

#### Scenario: Shown {#S1}
- **WHEN** a struct has `// spec: ENT-0001` above it
- **THEN** `entities ENT-0001` lists the file and line, the name `Order`, its kind and its fields, as text and as JSON, and the list gives the number of places in code; and when the entity changes `ci` names the code in its text, JSON and comment

#### Scenario: Suspect {#S2}
- **WHEN** the link is confirmed, and then the struct changes, and then the entity changes
- **THEN** each change makes the link suspect, and `confirm` makes it ok again

#### Scenario: Fields of a type {#S3}
- **WHEN** a Rust struct, a Go struct, a TypeScript interface, class and type alias and a JavaScript class have fields
- **THEN** each field name is found, a unit struct, a tuple struct, embedded Go fields and a function have none, and Go fields named together are each found
