---
id: REQ-MCP-0007
type: requirement
status: accepted
date: 2026-11-08
---
### Requirement: Nodes are resources and there are prompts
The system SHALL offer each node as an MCP resource and a few prompts, list the resources in pages of 500 so that every node appears once, and answer an unknown resource with an error.

#### Scenario: Resources and prompts {#S1}
- **WHEN** a client lists resources and prompts and reads one resource
- **THEN** it sees the capabilities, the nodes as resources with URIs, the text of the node, and an error for an unknown one

#### Scenario: Pages {#S2}
- **WHEN** a project has 530 nodes and the client follows the cursor
- **THEN** the first page has 500 and a cursor, the second has 30 and none, no node is listed twice, and a cursor past the end is an empty page

```claims
mcp.resources_page_size == 500
```
