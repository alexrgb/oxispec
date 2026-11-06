---
id: REQ-STYLE-0002
type: requirement
status: accepted
date: 2026-11-06
---
### Requirement: A node meets its format profile
The system SHALL report, with its own code for each, a node that lacks a section, a front matter field or an allowed status that its format profile requires, SHALL report a profile named in the config that does not exist or is for another type, and SHALL leave a node that meets its profile alone.

#### Scenario: Meets it {#S1}
- **WHEN** a node has every section and field its profile requires
- **THEN** nothing is reported

#### Scenario: Lacks something {#S2}
- **WHEN** a section, a field and a status are missing
- **THEN** E028, E029 and E030 are reported

#### Scenario: A wrong profile {#S3}
- **WHEN** the config names a profile that does not exist or is for another type
- **THEN** E031 is reported
