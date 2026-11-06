---
id: REQ-API-0002
type: requirement
status: accepted
date: 2026-11-06
---
### Requirement: A description is compared with the requirements
The system SHALL compare an OpenAPI or Swagger file with the operations the requirements state, and report each call that only one side has and each call whose response codes differ.

#### Scenario: Both sides {#S1}
- **WHEN** the file has a call with no requirement, and a call with codes the requirements do not state
- **THEN** both are reported, and a file made from the requirements has no difference

#### Scenario: A missing operation {#S2}
- **WHEN** a requirement states a call the file does not have
- **THEN** the call is reported as only in the requirements
