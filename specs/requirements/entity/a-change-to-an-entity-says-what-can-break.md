---
id: REQ-ENTITY-0003
type: requirement
status: accepted
date: 2026-11-08
---
### Requirement: A change to an entity says what can break
The system SHALL say what changed between two states of an entity (a rename, an attribute added or removed, a type changed, values added or removed), and mark a change as breaking when a reader of the entity can stop working: a removed or renamed thing, a changed type, or an enum value that is gone.

#### Scenario: Kinds of change {#S1}
- **WHEN** an entity changes an attribute type, drops and adds enum values, adds an attribute, loses an attribute and is renamed
- **THEN** each is named, and only the removed, renamed and retyped things and the values that are gone are breaking
