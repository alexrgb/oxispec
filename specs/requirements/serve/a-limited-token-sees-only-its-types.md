---
id: REQ-SERVE-0004
type: requirement
status: accepted
date: 2026-11-07
---
### Requirement: A limited token sees only its types
The system SHALL offer a token limited to some node types only search and get, return only nodes of those types, and answer as for a tool that does not exist to any other tool, without a count or a message that shows what is hidden.

#### Scenario: A token for decisions {#S1}
- **WHEN** a token limited to `adr` lists the tools, searches and asks for another type
- **THEN** it sees two tools, finds no requirement, gets no error that confirms a hidden type, and a call to another tool is answered as unknown
