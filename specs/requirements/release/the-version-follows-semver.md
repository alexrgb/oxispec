---
id: REQ-RELEASE-0001
type: requirement
status: accepted
date: 2026-11-06
---
### Requirement: The suggested version follows SemVer
The system SHALL suggest the next version from the kind of change: before 1.0 a breaking change raises the minor version, from 1.0 it raises the major version, and wording changes only raise the patch version.

#### Scenario: Before 1.0 {#S1}
- **WHEN** the last version is 0.3.1 and a breaking change was made
- **THEN** the suggestion is 0.4.0

#### Scenario: From 1.0 {#S2}
- **WHEN** the last version is 1.4.2
- **THEN** a major change gives 2.0.0, a minor 1.5.0 and a patch 1.4.3

#### Scenario: Kinds of change {#S3}
- **WHEN** only the wording of a requirement changed
- **THEN** the change is a patch and is listed as a text change
