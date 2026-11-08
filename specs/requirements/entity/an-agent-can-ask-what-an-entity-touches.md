---
id: REQ-ENTITY-0006
type: requirement
status: accepted
date: 2026-11-08
---
### Requirement: An agent can ask what an entity touches
The system SHALL offer the MCP tool `spec_entities` only when the consistency module is on, give the entities and what depends on them, and refuse an ID that could be read as an option or a depth outside the range.

#### Scenario: The tool {#S1}
- **WHEN** the module is on, and then off
- **THEN** the tool is listed only with the module, it returns the entity and its dependents, and a bad ID or depth is an error
