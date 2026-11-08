---
id: REQ-CONNECT-0002
type: requirement
status: accepted
date: 2026-11-08
---
### Requirement: Connect can point at a running server
The system SHALL write the address of a running server with `connect --url` and SHALL refer to the token by the name of an environment variable (or, for Copilot, ask for it), never write it into a file.

#### Scenario: An address {#S1}
- **WHEN** `connect --url` names a server
- **THEN** the clients' configs hold the address and `${OXISPEC_TOKEN}` (Copilot asks for it), no file holds a token, and a later `connect` without `--url` writes the local command again
