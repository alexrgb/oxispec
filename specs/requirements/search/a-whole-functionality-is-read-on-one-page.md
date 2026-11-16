---
id: REQ-SEARCH-0004
type: requirement
status: accepted
date: 2026-11-16
---
### Requirement: A whole functionality is read on one page
The system SHALL print every requirement of one functionality with its scenarios in one document, name the change sets still open for it, and list the functionalities there are.

#### Scenario: One functionality {#S1}
- **WHEN** `read` is run with the folder of some requirements (`authentication`) or the domain of their IDs (`AUTH`), in any case and with or without hyphens
- **THEN** each requirement is printed with its ID, status, path and full text, in the order of its number, under a heading that counts the requirements and scenarios, and `--status` keeps only the requirements of one status

#### Scenario: Open change sets {#S2}
- **WHEN** a change set that is not merged or reverted changes a requirement of the functionality
- **THEN** it is listed after the requirements with its status and how many tasks are done, and `--json` has the requirements with their text and the open change sets

#### Scenario: The functionalities {#S3}
- **WHEN** `read` is run with no name, or with a name no requirement has
- **THEN** the first lists the functionalities with their domain and how many requirements each has, and the second fails and names the functionalities there are
