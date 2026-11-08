---
id: REQ-HISTORY-0002
type: requirement
status: accepted
date: 2026-11-07
---
### Requirement: History needs a git repository
The system SHALL say that a git repository is needed for the commands that read history, and SHALL say that a node is created but not committed in a repository with no commits.

#### Scenario: No repository {#S1}
- **WHEN** `history`, `diff` or `show --at` is run in a folder that is not a git repository
- **THEN** the command exits with 2 and says it needs a git repository

#### Scenario: No commits {#S2}
- **WHEN** a node exists in a repository without commits
- **THEN** history says it was created and not committed

```claims
exit.refusal == 2
```
