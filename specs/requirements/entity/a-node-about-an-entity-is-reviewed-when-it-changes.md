---
id: REQ-ENTITY-0009
type: requirement
status: accepted
date: 2026-11-08
---
### Requirement: A node about an entity is reviewed when the entity changes
The system SHALL remember, when a person confirms it, what an entity said (its name and attributes, not the text around them) for each node that is `about` it, SHALL treat the node as needing review when the entity says something different, SHALL list it in `oxispec suspect` and warn about it with W026 in `oxispec ci`, mark it in `oxispec entities`, and SHALL settle it when the node is confirmed with `oxispec confirm NODE`. A node confirmed for an entity it is no longer about SHALL be dropped from the lock file.

#### Scenario: The states {#S1}
- **WHEN** a node about an entity is not confirmed, then confirmed, then the entity gets a new attribute, and then the node is confirmed
- **THEN** it is unconfirmed, ok, to review (with the warning W026 naming both), and ok again, a reworded description does not change anything, and confirming another node leaves it alone

#### Scenario: A link that is gone {#S2}
- **WHEN** a node that was confirmed is no longer about the entity
- **THEN** its confirmation is orphaned and a later `confirm` drops it, and a lock file from before reviews existed still reads

#### Scenario: The commands {#S3}
- **WHEN** an entity gets a new attribute after the nodes about it were confirmed
- **THEN** `suspect` exits with 1 and says W026, as JSON too; `entities` marks the node as needing review; `ci` warns and fails only with `--strict`; and `confirm NODE` settles it and writes it to the lock file
