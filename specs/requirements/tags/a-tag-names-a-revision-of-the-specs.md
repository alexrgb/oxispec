---
id: REQ-TAGS-0001
type: requirement
status: accepted
date: 2026-11-08
---
### Requirement: A tag names a revision of the specs
The system SHALL name a revision with `oxispec tag NAME` as a git tag `specs-NAME`, list the tags with their first line, accept a tag wherever a revision is accepted, and say when spec changes are left out of the tag because they are not committed.

#### Scenario: Tags {#S1}
- **WHEN** a tag is made on the first commit and another on the latest, and the list is asked as text and JSON
- **THEN** the list says there are none at first, then names both with their commit subjects, and the JSON has a schema version

#### Scenario: Uncommitted changes {#S2}
- **WHEN** a tag is made while a spec change is not committed
- **THEN** the command says the change is not committed and left out
