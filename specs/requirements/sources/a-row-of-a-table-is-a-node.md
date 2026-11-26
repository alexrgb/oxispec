---
id: REQ-SOURCES-0001
type: requirement
status: accepted
date: 2026-11-27
---
### Requirement: A row of a table is a node
The system SHALL let a project give `sources:`, a glob of documents and the columns of a table in them (ID with an optional pattern, type, title, links, fields), SHALL read each row that has an ID as a node of that type with its title, links, fields and the line it is on, so that `check`, `trace`, `coverage`, `show`, `list` and `search` treat it as any node, and SHALL refuse settings that cannot be used.

#### Scenario: A row is a node {#S1}
- **WHEN** a table has an ID column that matches the pattern, a title column, a column of IDs for `verifies` and a priority column
- **THEN** each row with an ID is a node with that ID, type, title, links and field, and the line of the row; a link cell with a range or a Markdown link gives plain IDs

#### Scenario: Other tables and rows {#S2}
- **WHEN** a document has a table that has no ID in the ID column, a table inside a code block, and a row without an ID in a table that does
- **THEN** the first two give no node, the row gives warning W030 at its line, and a range in a link cell is expanded

#### Scenario: Unusable settings {#S3}
- **WHEN** a column is 0, a pattern is not a regular expression, or the type is empty
- **THEN** the config is not read, and the message names the source and the key

#### Scenario: The commands read it {#S4}
- **WHEN** the config has a source, a table row `TC-CAT-001` verifies a node, and a test names `TC-CAT-001` in its title
- **THEN** `show` prints the row's line and file, `trace` lists the test, and `check` reports a link to a node that is missing
