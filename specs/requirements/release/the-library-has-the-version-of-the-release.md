---
id: REQ-RELEASE-0005
type: requirement
status: accepted
date: 2026-11-08
---
### Requirement: The library has the version of the release
The system SHALL keep the version of the `oxispec-core` dependency equal to the version of the release, and a test SHALL fail when they differ.

#### Scenario: Equal versions {#S1}
- **WHEN** the workspace version and the version of the library dependency are read
- **THEN** they are the same
