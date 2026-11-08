---
id: REQ-CONNECT-0001
type: requirement
status: accepted
date: 2026-11-08
---
### Requirement: Connect adds the server to the clients' config
The system SHALL add the MCP server to the config files of Claude Code, Cursor, Copilot and Gemini CLI, keep the other settings in them, show what it would do with `--dry-run`, and refuse a client it does not know and a file it cannot merge, writing nothing in that case.

#### Scenario: Writing {#S1}
- **WHEN** `connect` is run with `--dry-run` and then for real, with a settings file that has other keys
- **THEN** the dry run writes nothing, the real run adds the entry in each client's form and keeps the other keys

#### Scenario: Refusal {#S2}
- **WHEN** a client is unknown or a file has comments it cannot merge
- **THEN** the command exits with 2 and says to edit by hand, and no other file is written and the file is unchanged
