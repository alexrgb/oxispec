---
id: REQ-ROBUSTNESS-0001
type: requirement
status: accepted
date: 2026-11-08
---
### Requirement: No input makes a command panic
The system SHALL answer a hostile file (empty, binary, with a NUL byte, with a byte order mark or CRLF line ends, linking to itself, with an unfinished block, or with a line of 300,000 characters) with an error or a finding and never with a panic, and SHALL end quietly when the pipe it writes to is closed.

#### Scenario: Hostile files {#S1}
- **WHEN** the commands that read the specs run on a project with such files
- **THEN** no command panics and each exits with 0 or 1, the node with the byte order mark is found and a machine problem in a CRLF file is reported

#### Scenario: A closed pipe {#S2}
- **WHEN** the output goes to a program that stops reading
- **THEN** the program ends without a panic message
