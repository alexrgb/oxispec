---
id: REQ-DOCS-0001
type: requirement
status: accepted
date: 2026-11-08
---
### Requirement: The reference is written from the program
The system SHALL write the command reference, the check codes and the MCP tools reference from the program itself with `docs reference`, SHALL notice with `--check` when a committed file differs from what the program would write, and every relative link in the guide and the reference SHALL point at a file.

#### Scenario: Written from the program {#S1}
- **WHEN** `docs reference` is run
- **THEN** each command, each check code and each MCP tool is in the files, with the module a tool needs and whether it writes files

#### Scenario: The committed files {#S2}
- **WHEN** the reference in the repository is compared with the program
- **THEN** it is the same, and `--check` fails when a command changes

#### Scenario: Links {#S3}
- **WHEN** the links in the guide and the reference are read
- **THEN** each points at a file, and at least twenty were checked
