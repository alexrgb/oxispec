---
id: REQ-APIINDEX-0003
type: requirement
status: accepted
date: 2026-11-06
---
### Requirement: A query finds the nearest operations
The system SHALL answer a query with the operations whose vectors are nearest to the query's, nearest first, up to a count.

#### Scenario: Nearest first {#S1}
- **WHEN** a query shares words with one operation more than the others
- **THEN** that operation comes first, and a word and its plural meet
