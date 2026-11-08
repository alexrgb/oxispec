---
id: REQ-RELEASE-0006
type: requirement
status: accepted
date: 2026-11-08
---
### Requirement: Each published crate carries the license texts
The system SHALL include the texts of both licenses (`LICENSE-MIT` and `LICENSE-APACHE`) in each crate that is published, the same as the project's, because a crate is distributed on its own and both licenses ask for their text to travel with the code.

#### Scenario: The texts {#S1}
- **WHEN** the license files of `oxispec-core` and `oxispec` are compared with the project's
- **THEN** each is the same text, and a test fails when one differs or is missing
