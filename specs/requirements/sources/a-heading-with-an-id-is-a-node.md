---
id: REQ-SOURCES-0003
type: requirement
status: accepted
date: 2026-11-27
---
### Requirement: A heading with an ID is a node
The system SHALL let a source name `heading:` with an ID pattern and a type, SHALL read each heading outside a code block whose text has a match as a node with that ID and type, the rest of the heading as its title, and the lines from the heading to the next heading of the same or a higher level as its place, and SHALL treat it as read-only like a row of a table.

#### Scenario: A heading is a node {#S1}
- **WHEN** a document has `## US-CAT-001: Browse programs`, a `### Notes` below it, and then another `##` heading
- **THEN** `US-CAT-001` is a node titled `Browse programs` that runs to the line before that `##` heading, and a heading without an ID is not a node

#### Scenario: Code blocks and changes {#S2}
- **WHEN** an ID is in a heading inside a code block, or the text under one heading changes
- **THEN** the code block gives no node, and only the hash of the changed node changes

#### Scenario: The commands read it and do not write it {#S3}
- **WHEN** the config has a heading source, and `show --text`, `trace` and `link` name a story of it
- **THEN** `show` prints the lines of its section and `trace` lists the tests that name it, and `link` fails saying it is a section of a document, with the file and line
