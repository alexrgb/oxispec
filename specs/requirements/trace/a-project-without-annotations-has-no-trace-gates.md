---
id: REQ-TRACE-0006
type: requirement
status: accepted
date: 2026-11-08
---
### Requirement: A project without annotations has no trace gates
The system SHALL not hold a release back for suspect links or failing tests in a project that has no annotations.

#### Scenario: No annotations {#S1}
- **WHEN** a release is prepared in a project with no `spec:` or `verifies:` comments
- **THEN** the gate for suspect links is not listed and the release is not held
