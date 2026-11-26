---
id: REQ-FORMAT-0009
type: requirement
status: accepted
date: 2026-11-26
---
### Requirement: Frontmatter that is not YAML says where
The system SHALL report frontmatter that is not valid YAML as W002 at the line of the file where the error is, and SHALL say that the value needs quotes when the line has a second `: ` in a value.

#### Scenario: An unquoted colon {#S1}
- **WHEN** a title is written as `title: Erasure (Art. 17): a directory`
- **THEN** W002 names the line of the title in the file and says the value needs quotes
