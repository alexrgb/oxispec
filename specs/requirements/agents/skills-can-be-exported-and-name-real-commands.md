---
id: REQ-AGENTS-0003
type: requirement
status: accepted
date: 2026-11-08
---
### Requirement: Skills can be exported and name real commands
The system SHALL write the skills in the layout other tools read with `agents export`, SHALL keep the published skills folder equal to what the generator writes, and SHALL name only commands that exist.

#### Scenario: Export {#S1}
- **WHEN** `export` is run with `--all`, and in a project
- **THEN** it needs `--all` outside a project, and in a project it writes only the skills of the enabled modules

#### Scenario: Published skills {#S2}
- **WHEN** the skills folder of the repository is read
- **THEN** it matches what the generator writes

#### Scenario: Commands in skills {#S3}
- **WHEN** the commands a skill names are looked up
- **THEN** each one is a command of the program
