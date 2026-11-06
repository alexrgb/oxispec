---
id: REQ-RELEASE-0003
type: requirement
status: accepted
date: 2026-11-06
---
### Requirement: Release notes list what changed
The system SHALL write release notes with a section for each kind of change that has content, and SHALL say when it is the first release.

#### Scenario: Sections {#S1}
- **WHEN** a release has breaking and changed items, no added items and one change set
- **THEN** the notes have the Breaking, Changed and Change sets sections and no Added section
