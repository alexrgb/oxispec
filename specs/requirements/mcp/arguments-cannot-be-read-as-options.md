---
id: REQ-MCP-0003
type: requirement
status: accepted
date: 2026-11-05
---
### Requirement: Arguments cannot be read as options
The system SHALL refuse a tool argument that git or oxispec could read as an option, a malformed ID, a missing required argument and a number outside its limits.

#### Scenario: Arguments that look like options {#S1}
- **WHEN** a client passes `--output=x` as a revision, or `--help` as an ID
- **THEN** the call fails with an error result and nothing is run

#### Scenario: Words that look like options in a search {#S2}
- **WHEN** a client searches for the words `--help`
- **THEN** it is a search for those words and succeeds
