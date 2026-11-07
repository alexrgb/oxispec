---
id: REQ-MCP-0005
type: requirement
status: accepted
date: 2026-11-07
---
### Requirement: An agent can search the operations of the API
The system SHALL offer the tool `spec_api_search`, which answers with the data of `oxispec api search --json`, SHALL report an error the client can read when there is no index. A caller whose token is limited to some node types SHALL NOT be offered it, because the text of an operation draws on nodes of every type.

#### Scenario: A question {#S1}
- **WHEN** the operations are indexed and a client calls `spec_api_search` with the query "charge a card"
- **THEN** the answer lists `POST /payments` first, and without an index the result is an error that says to run `oxispec api index`

#### Scenario: A limited token {#S2}
- **WHEN** a client with a token limited to one node type lists the tools and calls `spec_api_search`
- **THEN** the tool is not listed and the call is answered as for a tool that does not exist
