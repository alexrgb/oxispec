---
id: REQ-RELEASE-0004
type: requirement
status: accepted
date: 2026-11-08
---
### Requirement: Releases need their module and a repository
The system SHALL refuse the release commands when the module is off or the folder is not a git repository, and say which, and say that there are no releases yet when there are none.

#### Scenario: No module or repository {#S1}
- **WHEN** `release` is run without the module, and then in a folder that is not a repository
- **THEN** each exits with 2 and says what is missing, and with the module `release list` says there are no releases yet
