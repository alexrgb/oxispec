---
id: REQ-ROLLBACK-0004
type: requirement
status: accepted
date: 2026-11-08
---
### Requirement: A rollback puts nodes back without committing
The system SHALL put a node, or every node with `--all`, back to how it was at a revision, recreate a node that was deleted, move a file back to its old path, show what it would do with `--dry-run` or as JSON without changing anything, commit nothing, and refuse bad input or a folder that is not a git repository.

#### Scenario: A node and its path {#S1}
- **WHEN** a node is rolled back to the first commit
- **THEN** its text and status are those of that commit and its file is at the old path, and nothing is committed

#### Scenario: Deleted nodes and all {#S2}
- **WHEN** a deleted node is rolled back, and then everything to the first commit
- **THEN** the file is back, one node is restored and one removed, and the diff against that commit is empty

#### Scenario: A dry run {#S3}
- **WHEN** the rollback is run with `--dry-run` and with `--json`
- **THEN** the actions are listed with `schema_version` and `dry_run`, and the working tree is unchanged

#### Scenario: Bad input {#S4}
- **WHEN** no node is named, a node and `--all` are given together, a node or a revision does not exist, or the folder is not a repository
- **THEN** the command exits with 2 and says what is wrong (name the nodes, not both, no such node, unknown revision, needs a git repository), and no file changes
