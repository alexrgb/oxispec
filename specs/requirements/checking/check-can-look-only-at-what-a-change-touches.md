---
id: REQ-CHECKING-0001
type: requirement
status: accepted
date: 2026-11-08
---
### Requirement: Check can look only at what a change touches
The system SHALL, with `--changed`, check the nodes that changed and the nodes that link to them against a base (the last commit unless `--base` names another), SHALL report an error in a node that did not change itself when a change elsewhere broke its link, SHALL check everything in a repository with no commits, and SHALL say that a git repository is needed outside one.

#### Scenario: What a change touches {#S1}
- **WHEN** an old error exists, a node is edited, and then a scenario anchor is renamed
- **THEN** the old error is not reported, an edit checks the node and the nodes that link to it, and the renamed anchor reports E009 in the node that links to it

#### Scenario: Another base {#S2}
- **WHEN** `--base` names an older commit, and then a ref that does not exist
- **THEN** the older commit is the base, and the unknown ref exits with 2

#### Scenario: No commits and no repository {#S3}
- **WHEN** `--changed` is run in a repository with no commits, and in a folder that is not a repository
- **THEN** the first checks everything, and the second exits with 2 and says it needs a git repository

```claims
exit.refusal == 2
```
