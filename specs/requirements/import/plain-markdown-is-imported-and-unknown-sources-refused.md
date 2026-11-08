---
id: REQ-IMPORT-0003
type: requirement
status: accepted
date: 2026-11-08
---
### Requirement: Plain markdown is imported and unknown sources are refused
The system SHALL import plain markdown files as nodes, ask for `--dir` when it is needed, and refuse a source it does not know.

#### Scenario: Plain markdown {#S1}
- **WHEN** `import specs --from markdown` is run with and without `--dir`, and with a source that does not exist
- **THEN** without `--dir` it fails and names `--dir`, with it the files become nodes, and an unknown source says `unknown source`
