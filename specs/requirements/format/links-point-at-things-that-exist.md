---
id: REQ-FORMAT-0003
type: requirement
status: accepted
date: 2026-11-05
links:
  decided-by:
    - ADR-0002
---
### Requirement: Links point at things that exist
The system SHALL report an error for a link whose target is not a node of the project, and for a link to a scenario that the target does not have. A link to a node of another project is reported as information and is not checked.

#### Scenario: A link to a missing node {#S1}
- **WHEN** a node links to an ID that no file declares
- **THEN** the finding E002 is reported

#### Scenario: A link to a missing scenario {#S2}
- **WHEN** a node links to `REQ-A-0010.S9` and that requirement has no scenario with the anchor S9
- **THEN** the finding E009 is reported and names the anchor

#### Scenario: A link to another project {#S3}
- **WHEN** a link target has the form `platform/identity:ADR-0009`
- **THEN** only the information I001 is reported
