---
id: REQ-SEARCH-0003
type: requirement
status: accepted
date: 2026-11-12
---
### Requirement: Nodes are listed and shown with their text
The system SHALL list the nodes of a type, a status or a domain, and SHALL show the text of a node on request.

#### Scenario: A list {#S1}
- **WHEN** nodes are listed by type and by the domain of their ID
- **THEN** only the nodes of that type and domain are listed, with their status and title

#### Scenario: The text {#S2}
- **WHEN** a node is shown with `--text`
- **THEN** its text is printed without the front matter
