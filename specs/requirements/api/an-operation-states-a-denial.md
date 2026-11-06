---
id: REQ-API-0006
type: requirement
status: accepted
date: 2026-11-06
---
### Requirement: An operation says what happens to a caller who may not use it
The system SHALL warn about an operation that has no scenario responding with 401 or 403 in a project where other operations have one, and SHALL leave alone an operation whose statement says it is public, and a project where no operation models a denial.

#### Scenario: A gap {#S1}
- **WHEN** one operation responds with 403 and another responds only with 200
- **THEN** the finding W023 names the second, and says nothing about an operation that is public, or when no operation responds with 401 or 403
