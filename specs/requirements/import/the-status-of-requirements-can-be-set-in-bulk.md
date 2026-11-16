---
id: REQ-IMPORT-0005
type: requirement
status: accepted
date: 2026-11-16
---
### Requirement: The status of requirements can be set in bulk
The system SHALL set the status of requirements by ID or for a whole functionality, only to a status the requirement profile allows, and only for requirements, with a dry run that writes nothing.

#### Scenario: A functionality {#S1}
- **WHEN** `set-status accepted --domain authentication` is run for a functionality whose requirements are drafts
- **THEN** each requirement's `status` becomes `accepted`, the report names each change and the count, and the rest of the files is left as it was

#### Scenario: Only from a status {#S2}
- **WHEN** `--from draft` is given
- **THEN** only requirements that are drafts change, the others are counted as left, and one that already has the status is counted as such

#### Scenario: A dry run {#S3}
- **WHEN** `--dry-run` is given
- **THEN** the report says what would change and nothing is written

#### Scenario: Refused {#S4}
- **WHEN** the status is not one the profile allows, an ID is not a requirement, or neither IDs nor `--domain` is given
- **THEN** the command fails with the reason and nothing is written
