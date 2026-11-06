---
id: REQ-SUSPECT-0001
type: requirement
status: accepted
date: 2026-11-06
---
### Requirement: A link goes suspect when either side changes
The system SHALL mark a confirmed link between a requirement and code as suspect when the text of the requirement changes, when the code changes, or both, SHALL NOT count formatting or comments as a change, and SHALL report a link whose annotation is gone as orphaned until it is confirmed again.

#### Scenario: Unconfirmed and ok {#S1}
- **WHEN** a link is new, then confirmed, then the code is reformatted or commented
- **THEN** it is unconfirmed, then ok, and still ok after the reformatting

#### Scenario: Either side {#S2}
- **WHEN** the code, the requirement text, or both change after the confirmation
- **THEN** the link is suspect on the code side, the spec side, or both

#### Scenario: A removed annotation {#S3}
- **WHEN** the annotation of a confirmed link is removed
- **THEN** the link is orphaned, and confirming again drops it
