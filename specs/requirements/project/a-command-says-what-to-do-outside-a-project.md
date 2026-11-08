---
id: REQ-PROJECT-0003
type: requirement
status: accepted
date: 2026-11-08
---
### Requirement: A command says what to do outside a project
The system SHALL tell a user who runs a command outside a project to run `oxispec init`, and SHALL refuse a project whose `format` is newer than the program knows.

#### Scenario: No project {#S1}
- **WHEN** a command is run in a folder with no config
- **THEN** the command exits with 2 and names `oxispec init`

#### Scenario: A newer project {#S2}
- **WHEN** the config says `format: 99`
- **THEN** every command exits with 2 and names the format
