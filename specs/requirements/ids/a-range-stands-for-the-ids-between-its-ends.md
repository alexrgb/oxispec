---
id: REQ-IDS-0002
type: requirement
status: accepted
date: 2026-11-26
---
### Requirement: A range stands for the IDs between its ends
The system SHALL read `TC-CAT-1200..1212` and `TC-CAT-1200..TC-CAT-1212` as the IDs from one end to the other, keeping leading zeros, in a test title found by `trace.test_ids` and in a link of a node, and SHALL not read a range that runs backwards, mixes two prefixes or stands for more than 1000 IDs.

#### Scenario: The IDs between the ends {#S1}
- **WHEN** a range is `TC-CAT-1200..1203`, `TC-CAT-1200..TC-CAT-1201` or `TC-A-008..010`
- **THEN** it stands for each ID from the first to the last, `TC-A-009` among them in the last case

#### Scenario: Not a range {#S2}
- **WHEN** the ends run backwards, have different prefixes, or are more than 1000 IDs apart
- **THEN** the text is not a range

#### Scenario: A test title names a range {#S3}
- **WHEN** a test is titled `TC-CAT-1200..1202 filters`, and nodes with those three IDs exist
- **THEN** the test verifies all three

#### Scenario: A link names a range {#S4}
- **WHEN** a node links `verifies` to `TC-CAT-1200..1202`
- **THEN** each ID of the range is checked as a link of its own
