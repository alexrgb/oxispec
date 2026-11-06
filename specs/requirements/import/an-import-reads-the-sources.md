---
id: REQ-IMPORT-0001
type: requirement
status: accepted
date: 2026-11-06
---
### Requirement: An import reads the specs of other tools
The system SHALL read specs written for OpenSpec, Spec Kit, Kiro, as a product requirements document or as an OpenAPI description into requirement drafts with their scenarios.

#### Scenario: OpenSpec {#S1}
- **WHEN** a spec has requirements with scenarios
- **THEN** each requirement and its WHEN, THEN and AND steps are kept

#### Scenario: Kiro {#S2}
- **WHEN** a requirement has acceptance criteria written WHEN ... THEN
- **THEN** each criterion is a scenario split at THEN

#### Scenario: Spec Kit {#S3}
- **WHEN** a spec has user stories and FR-001 lines
- **THEN** stories and functional requirements become requirements

#### Scenario: A PRD {#S4}
- **WHEN** a PRD has bullets under a requirements section and sections such as non-goals
- **THEN** the bullets become requirements with their criteria and the other sections are listed as left out

#### Scenario: OpenAPI {#S5}
- **WHEN** an OpenAPI file has operations with tags and responses
- **THEN** a draft is made for each tag with a requirement per operation and a scenario per response
