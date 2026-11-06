---
id: REQ-RELEASE-0002
type: requirement
status: accepted
date: 2026-11-06
---
### Requirement: Gates hold a release back
The system SHALL hold a release back while a change set it includes is not approved, while the specs have errors, and while there are suspect links or failing tests, and SHALL let a clean release through.

#### Scenario: A clean release {#S1}
- **WHEN** the change sets are approved and nothing is broken
- **THEN** every gate passes

#### Scenario: Not approved or broken {#S2}
- **WHEN** a change set is in review, or a link points at a node that does not exist
- **THEN** the release is held back and the gate says which change set

#### Scenario: Suspect or failing {#S3}
- **WHEN** the project uses trace and has suspect links or failing tests
- **THEN** the release is held back, and a project without trace has no such gates

#### Scenario: The first release {#S4}
- **WHEN** there is no earlier version and no change set
- **THEN** the release is allowed
