---
id: REQ-AGENTS-0001
type: requirement
status: accepted
date: 2026-11-08
---
### Requirement: Skills are written for the clients a project chooses
The system SHALL write skills for Claude Code, Cursor, Copilot and Gemini CLI at `init`, or for the clients named or for none, only for the modules that are enabled, SHALL refuse a client it does not know, and SHALL list the clients and skills.

#### Scenario: The default {#S1}
- **WHEN** `init` is run with no choice
- **THEN** the skills for the four clients are written, the shared agents folder is not, and a skill for a module that is off is not

#### Scenario: A choice {#S2}
- **WHEN** `init` names clients, names none, or names an unknown client
- **THEN** the named clients get skills, none writes nothing and leaves the command for later, and an unknown client or `none` with others exits with 2

#### Scenario: The list {#S3}
- **WHEN** `agents list` is run
- **THEN** it marks the selected clients, names the skills, gives the same as JSON, and refuses a bad client name
