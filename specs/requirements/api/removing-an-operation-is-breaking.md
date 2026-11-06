---
id: REQ-API-0004
type: requirement
status: accepted
date: 2026-11-06
---
### Requirement: Removing from the API is a breaking change
The system SHALL count the removal of an operation or of one of its responses as a breaking change in a release, and the addition of an operation as a minor one.

#### Scenario: Removal and addition {#S1}
- **WHEN** a release removes an operation or a response code, or adds an operation
- **THEN** the removal is breaking and needs a major version (a minor one before 1.0), and the addition needs a minor one
