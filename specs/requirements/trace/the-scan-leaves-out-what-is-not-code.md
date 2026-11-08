---
id: REQ-TRACE-0005
type: requirement
status: accepted
date: 2026-11-08
---
### Requirement: The scan leaves out what is not code
The system SHALL leave out of the trace scan the paths the project excludes, markdown files (they are the specs) and files git ignores.

#### Scenario: Excluded files {#S1}
- **WHEN** markers sit in an excluded folder, a git-ignored folder and a markdown file, and in two code files
- **THEN** only the two annotations in code are counted
