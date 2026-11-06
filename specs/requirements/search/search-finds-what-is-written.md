---
id: REQ-SEARCH-0001
type: requirement
status: accepted
date: 2026-11-06
---
### Requirement: Search finds what is written
The system SHALL find the nodes that hold every word asked for in their text, title or ID, rank a title match above a text match, show a snippet with the matched words marked, and narrow the results by type and status.

#### Scenario: Words {#S1}
- **WHEN** a query names words of a node's text, a decision's title, and a node ID
- **THEN** each is found, a word that matches nothing finds nothing, every word has to match, and a stray quote does not break the query

#### Scenario: Ranking {#S2}
- **WHEN** one node has the word in its title and another only in its text
- **THEN** the title match comes first with the higher score

#### Scenario: Snippets {#S3}
- **WHEN** a word is found in the text
- **THEN** the snippet is one line with the matched word marked

#### Scenario: Filters {#S4}
- **WHEN** the query names a type, a status or both
- **THEN** only the nodes of that type and status are returned
