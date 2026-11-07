---
id: REQ-HISTORY-0001
type: requirement
status: accepted
date: 2026-11-07
---
### Requirement: History follows a node through git
The system SHALL list the changes of a node from git history, newest first, with the commit, what changed in the front matter and the text, the moves of its file, any edit not yet committed, and for a node that is gone, the revision named with `--at`, and SHALL offer the same as JSON with a limit that says when more history exists.

#### Scenario: The changes {#S1}
- **WHEN** a node was created, accepted with a link added, and its file renamed, then edited without a commit
- **THEN** each change is listed newest first with its commit subject, the status change and the link, the move, and the uncommitted edit at the top

#### Scenario: JSON and limit {#S2}
- **WHEN** history is asked as JSON, and with a limit of 1
- **THEN** the entries have an event, a commit and the changes, and the answer says it is truncated

#### Scenario: A deleted node {#S3}
- **WHEN** the node was removed in a later commit
- **THEN** the command asks for `--at` and with `--at` of an earlier revision it lists the history
