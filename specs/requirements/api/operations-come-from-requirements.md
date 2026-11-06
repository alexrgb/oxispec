---
id: REQ-API-0001
type: requirement
status: accepted
date: 2026-11-06
---
### Requirement: Operations come from the requirements
The system SHALL treat a requirement block titled as a call, such as `GET /orders/{id}`, as an API operation, with each scenario that says the API responds with a status code as a response, and SHALL write an OpenAPI skeleton of them that has no schemas or security.

#### Scenario: A call and its responses {#S1}
- **WHEN** a requirement block is titled `GET /orders/{id}` and its scenarios say the API responds with 200 and 404
- **THEN** the skeleton has that path and method with both responses, and a path parameter for `{id}`

#### Scenario: Not a call {#S2}
- **WHEN** a requirement block has a title that is not a method and a path
- **THEN** it is not an operation and is left out of the skeleton
