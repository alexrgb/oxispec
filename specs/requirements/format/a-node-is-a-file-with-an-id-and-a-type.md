---
id: REQ-FORMAT-0001
type: requirement
status: accepted
date: 2026-11-05
links:
  decided-by:
    - ADR-0001
---
### Requirement: A node is a file with an id and a type
The system SHALL read a markdown file as a node when its front matter has both `id` and `type`, SHALL ignore a markdown file whose front matter has neither, and SHALL report a file that has only one of them.

#### Scenario: A file without front matter is not a node {#S1}
- **WHEN** a markdown file such as a README has no front matter
- **THEN** it is not a node and no finding is reported

#### Scenario: Front matter without id and type is not a node {#S2}
- **WHEN** a file has front matter with other keys only
- **THEN** it is not a node and no finding is reported

#### Scenario: Only one of id and type {#S3}
- **WHEN** a file has an `id` but no `type`
- **THEN** the file is not a node and the finding E003 is reported
