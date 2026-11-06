---
id: REQ-APIINDEX-0004
type: requirement
status: accepted
date: 2026-11-06
---
### Requirement: The index is built and searched from the command line
The system SHALL build and update the index with `oxispec api index`, answer `oxispec api search` from it with the built-in embedder or with the service named in the config, and refuse a search when there is no index, when the index was made with another embedder, or when the service does not answer.

#### Scenario: The built-in embedder {#S1}
- **WHEN** the operations are indexed and then searched, with no search before the index
- **THEN** the first search says there is no index, a second index run reports the unchanged operations, and a query finds the nearest operation

#### Scenario: A service {#S2}
- **WHEN** the config names an embedding service, a search is made with the old index, then the index is built again and searched, and then the service is gone
- **THEN** the old index is refused, the new one finds the nearest operation, and the missing service is an error that names the embedder
