---
id: REQ-AGENTS-0002
type: requirement
status: accepted
date: 2026-11-08
---
### Requirement: Skills do not drift and hand edits are kept
The system SHALL write nothing for agents when the module is off and say how to add it, SHALL report a missing, outdated or edited skill with `agents check`, SHALL restore it with `agents update` while keeping a hand edit unless `--force` is given, and SHALL add project notes to the files it writes.

#### Scenario: No module {#S1}
- **WHEN** the agents module is off
- **THEN** nothing is written and the command says to add the module

#### Scenario: Drift {#S2}
- **WHEN** a file is missing and another is edited by hand
- **THEN** `check` exits with 1, `update` restores the missing one and keeps the edit, and `--force` replaces it

#### Scenario: Notes {#S3}
- **WHEN** a project note is added to files that are untouched
- **THEN** `check` reports the drift, `update` refreshes the files with the note and `check` then passes
