---
id: REQ-DIFF-0002
type: requirement
status: accepted
date: 2026-11-06
---
### Requirement: Graphs are compared by node
The system SHALL report the nodes added, removed and changed between two graphs, SHALL report the errors and warnings that are new and those that are fixed, and SHALL NOT call a finding new because its file moved.

#### Scenario: Nodes {#S1}
- **WHEN** a node is added, one removed and one changed
- **THEN** each appears under its kind

#### Scenario: Findings {#S2}
- **WHEN** a finding exists only in the new graph, and another only in the old
- **THEN** the first is new and the second is fixed

#### Scenario: A move {#S3}
- **WHEN** a file moves and keeps its findings
- **THEN** none of them is new
