---
id: REQ-CHANGE-0005
type: requirement
status: accepted
date: 2026-11-16
---
### Requirement: A change set keeps its tasks
The system SHALL read the checkboxes under the `## Tasks` heading of a change set as its tasks, show how many are done, show which tasks name a requirement or a scenario, and warn about a task that names what does not exist and about a merged change set with tasks still open.

#### Scenario: Progress {#S1}
- **WHEN** a change set has tasks, some of them ticked (`- [x] 1.1 Guard`)
- **THEN** `show` prints `tasks: 1/2 done` with each task, `change list` adds `[1/2 tasks]`, and `change list --json` has `tasks` with `done` and `total`

#### Scenario: Tasks that name a node {#S2}
- **WHEN** a task writes the ID of a requirement or of one of its scenarios, such as `REQ-AUTH-0001.S2`
- **THEN** `show` of that requirement lists the tasks that name it, with the change set each belongs to and whether it is done

#### Scenario: A task names what does not exist {#S3}
- **WHEN** a task names a requirement that is not in the graph, or a scenario its requirement does not have
- **THEN** `check` warns (W027), while a word that only looks like an ID of another kind of node (`TC-POST-040` where no such node exists) is left alone

#### Scenario: Merged with tasks open {#S4}
- **WHEN** a change set is `merged` and some of its tasks are not ticked
- **THEN** `check` warns (W028) with how many are open, and a change set with every task ticked, or with no tasks, has nothing to say
