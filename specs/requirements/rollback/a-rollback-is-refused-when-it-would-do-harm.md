---
id: REQ-ROLLBACK-0003
type: requirement
status: accepted
date: 2026-11-08
---
### Requirement: A rollback is refused when it would do harm
The system SHALL refuse a rollback that would give a new error, such as a broken link, and one that would overwrite work that is not committed, unless `--force` is given, and SHALL change nothing when it refuses.

#### Scenario: A broken link {#S1}
- **WHEN** a rollback would delete a node that another node links to
- **THEN** the command exits with 2, says what would break and how many new errors it gives, and no file changes

#### Scenario: Uncommitted work {#S2}
- **WHEN** a node has changes that are not committed and a rollback would replace them
- **THEN** the command exits with 2 and says the work is not committed, and with `--force` it goes ahead
