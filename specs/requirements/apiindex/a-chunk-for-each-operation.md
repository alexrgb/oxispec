---
id: REQ-APIINDEX-0001
type: requirement
status: accepted
date: 2026-11-06
---
### Requirement: Each operation has a chunk of text
The system SHALL make for each operation one chunk of text that holds its call, summary and scenarios, the product requirements document, decisions, tech debt, risks and test plans around it, and what an OpenAPI file says about it when one is given.

#### Scenario: The chunk {#S1}
- **WHEN** an operation has a PRD, a risk that it mitigates, a scenario and a description file with an operation ID and a parameter
- **THEN** the chunk holds all of them under the call
