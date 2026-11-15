---
id: REQ-PROJECT-0005
type: requirement
status: accepted
date: 2026-11-15
---
### Requirement: The folder of the documents can be changed at any time
The system SHALL move the documents of a project to another folder on request and set `docs_dir` to it, SHALL leave IDs, links and the contents of the files as they are, and SHALL refuse a move that would clash with what is in the way before it moves anything.

#### Scenario: A move {#S1}
- **WHEN** `oxispec move-docs docs/spec` is run in a project with documents
- **THEN** the folders of the documents are in `docs/spec`, the config says so, `check` finds the same nodes, and a new node is written there

#### Scenario: A look first {#S2}
- **WHEN** the move is run with `--dry-run`
- **THEN** it lists what would move and changes nothing

#### Scenario: Back to the old folder {#S3}
- **WHEN** the documents of a project from before (no `docs_dir`) are moved to `oxispec` and then back to `specs`
- **THEN** the architecture model goes from `architecture/model` to `oxispec/architecture` and back, and the config has no `docs_dir` again

#### Scenario: What is in the way {#S4}
- **WHEN** the new folder is the same as the old, one is inside the other, is not a folder of the project, or already has a folder that would be overwritten
- **THEN** the move is refused, nothing has moved, and the config is as it was

#### Scenario: Files that name the old folder {#S5}
- **WHEN** a file outside the documents says `specs/` in its text
- **THEN** the move lists that file and does not change it
