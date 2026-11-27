---
id: REQ-SOURCES-0002
type: requirement
status: accepted
date: 2026-11-27
---
### Requirement: A row of a table is read-only
The system SHALL refuse a command that writes a node (`link`, `set-status`, the debt and risk `status` commands and `rollback`) when the node is a row of a table, with a message that names the node, the file and the line, and SHALL leave such nodes out of `rollback --all`.

#### Scenario: Writing is refused {#S1}
- **WHEN** `oxispec link`, `oxispec set-status` or `oxispec rollback` names a node that is a row of a table
- **THEN** the command fails, the document is not changed, and the message says the node is a row and where

#### Scenario: Rollback leaves the document alone {#S2}
- **WHEN** `oxispec rollback --all` or `rollback REV ID` runs and rows of a table or sections are nodes
- **THEN** they are not listed in what is restored, naming one is refused, and their document is not changed
