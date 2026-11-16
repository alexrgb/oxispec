---
id: REQ-FORMAT-0008
type: requirement
status: accepted
date: 2026-11-16
---
### Requirement: The requirements of a domain share one folder
The system SHALL warn with W029 when requirements whose IDs use one domain are in more than one folder under `requirements/`, naming the folders.

#### Scenario: A domain split in two folders {#S1}
- **WHEN** requirements of the domain `RISK` are in `risk/` and `risks/`, and then all of them are in `risks/`
- **THEN** the first reports W029 once, for the folder after the first, and the second reports nothing
