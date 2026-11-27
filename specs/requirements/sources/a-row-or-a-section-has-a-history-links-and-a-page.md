---
id: REQ-SOURCES-0004
type: requirement
status: accepted
date: 2026-11-27
---
### Requirement: A row or a section has a history, suspect links and a page
The system SHALL read the rows and sections of `sources:` in the documents of a revision too, so that `history` lists the commits that changed that row or section and not the others in the same file, `diff` and `--at` compare them between revisions, a link confirmed to one is suspect when its text changes, and `site` writes a page for each with its fields, or its own lines for a section.

#### Scenario: History of one row {#S1}
- **WHEN** a commit changes one row of a table and another commit changes a different row of it
- **THEN** `oxispec history` on the first row lists the first commit and not the second

#### Scenario: Diff between revisions {#S2}
- **WHEN** a row is added, one is changed and the document is compared with an earlier revision
- **THEN** `oxispec diff` lists the added row and the changed one with their changes, and `show --at` finds a row that existed then

#### Scenario: A link goes suspect when the row changes {#S3}
- **WHEN** a link from a test to a row is confirmed and the text of the row then changes
- **THEN** the link is suspect on the spec side, and a cell change that is only formatting is not a change

#### Scenario: A page for each {#S4}
- **WHEN** the site is written for a project with a row and a section as nodes
- **THEN** the row's page shows its type, fields and `file:line` without the rest of the table, and the section's page shows its own lines and not the rest of the document
