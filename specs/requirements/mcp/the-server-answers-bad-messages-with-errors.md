---
id: REQ-MCP-0006
type: requirement
status: accepted
date: 2026-11-08
---
### Requirement: The server answers bad messages with errors
The system SHALL refuse to start `oxispec mcp` when the module is off and say how to add it, and SHALL answer a message that cannot be read, a method it does not know and a call with bad parameters with the JSON-RPC error codes -32700, -32601 and -32602.

#### Scenario: Bad messages {#S1}
- **WHEN** the server is started with and without the module and is sent bad messages
- **THEN** without the module it exits with 2 and names `modules add mcp`, and with it the replies carry the three error codes and the ID of the request
