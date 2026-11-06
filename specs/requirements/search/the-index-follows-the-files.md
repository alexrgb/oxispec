---
id: REQ-SEARCH-0002
type: requirement
status: accepted
date: 2026-11-06
---
### Requirement: The search index follows the files
The system SHALL keep the search index in step with the files by redoing only what changed, SHALL rebuild an index that is damaged or from an older version, and SHALL let searches at the same time share one index without errors.

#### Scenario: Only the changed {#S1}
- **WHEN** a stored index exists and one file changes
- **THEN** only that file is read again

#### Scenario: A damaged index {#S2}
- **WHEN** the stored index is damaged or from an older version
- **THEN** it is rebuilt and the search still answers

#### Scenario: At the same time {#S3}
- **WHEN** several searches run at once
- **THEN** they share one index and none fails
