---
id: REQ-REVISIONS-0001
type: requirement
status: accepted
date: 2026-11-08
---
### Requirement: A revision can be named in many ways
The system SHALL accept as a revision a branch, a tag (with or without the prefix `specs-`), a hash, a relative name such as `HEAD~1` and a date as `date:YYYY-MM-DD`, in `--at` and wherever a revision is asked for, and SHALL show and check the specs as they were then.

#### Scenario: Names {#S1}
- **WHEN** `show --at` is given each kind of revision
- **THEN** each gives the node as it was at that revision

#### Scenario: An old state {#S2}
- **WHEN** a node is shown and the project is checked at an older revision
- **THEN** the old status and file are shown, and the check finds what that revision had, not what is there now
