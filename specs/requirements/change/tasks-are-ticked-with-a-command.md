---
id: REQ-CHANGE-0006
type: requirement
status: accepted
date: 2026-11-16
---
### Requirement: Tasks are ticked with a command
The system SHALL tick and untick the tasks of a change set by their number, their place in the list or all at once, changing only the checkbox on their lines, and say how many are done.

#### Scenario: Ticking {#S1}
- **WHEN** `change task done CHG-0001 1.1 #3` is run
- **THEN** the checkboxes of task 1.1 and of the third task become `[x]`, the rest of the file is as it was, and the report says which were ticked and how many of the tasks are done

#### Scenario: Unticking and all {#S2}
- **WHEN** `change task undo CHG-0001 --all` is run
- **THEN** every `[x]` or `[X]` becomes `[ ]`, and a task that already is as asked is not touched

#### Scenario: Refused {#S3}
- **WHEN** a task is not in the change set, a number is used by two tasks, no task is named, the node is not a change set or it has no tasks
- **THEN** the command fails with the reason, naming the tasks there are where that helps, and nothing is written
