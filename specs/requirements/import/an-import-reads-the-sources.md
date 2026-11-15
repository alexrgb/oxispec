---
id: REQ-IMPORT-0001
type: requirement
status: accepted
date: 2026-11-06
---
### Requirement: An import reads the specs of other tools
The system SHALL read specs written for OpenSpec, Spec Kit, Kiro, as a product requirements document or as an OpenAPI description into requirement nodes, one for each requirement, with their scenarios.

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

#### Scenario: Many requirements in one file {#S6}
- **WHEN** a source file has several requirements with scenarios
- **THEN** each requirement is a node of its own with its scenarios numbered from S1, the purpose of the file is in the first node, and `check` has nothing to say about the result

#### Scenario: A status {#S7}
- **WHEN** specs are imported with `--status accepted`
- **THEN** the requirement nodes have that status

#### Scenario: The ID of the source {#S8}
- **WHEN** an OpenSpec or markdown requirement begins with an ID such as `ANAL-2`
- **THEN** its node is `REQ-ANAL-2` with `ANAL-2` as an alias, a number that is taken gives the next free one with a note, and a requirement without an ID gets a number nobody asked for

#### Scenario: Domains of choice {#S9}
- **WHEN** the config has `domains:` that map the name of a spec, or the prefix of a source ID, to a domain
- **THEN** the folder of a spec, the IDs of requirements with no ID in the source, and the IDs under a source prefix use that domain, the source ID stays as the alias, a delta still finds its requirements, and a domain that is not letters and digits, in parts joined by hyphens, is refused

#### Scenario: Names keep their hyphens {#S10}
- **WHEN** a spec has a name such as `account-and-profile` and the config gives it no domain
- **THEN** its folder is `account-and-profile` and its requirements are `REQ-ACCOUNT-AND-PROFILE-0001`, and a domain in `domains:` may have hyphens too when each part starts with a letter
