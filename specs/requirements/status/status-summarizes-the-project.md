---
id: REQ-STATUS-0001
type: requirement
status: accepted
date: 2026-11-08
---
### Requirement: Status summarizes the project
The system SHALL summarize the project with `oxispec status`: the number of nodes, the nodes by type and status, and the findings, as text and as JSON.

#### Scenario: The summary {#S1}
- **WHEN** `status` is run in a project with requirements and a decision, and outside a project
- **THEN** it gives the counts as text and JSON, and outside a project exits with 2
