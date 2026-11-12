---
id: REQ-ADR-0002
type: requirement
status: accepted
date: 2026-11-08
---
### Requirement: Decisions are imported from other tools
The system SHALL import decision records written for adr-tools, MADR and Log4brains as nodes with their status, date and supersede links, SHALL show what it would do with `--dry-run`, SHALL refuse to overwrite on a second import, and the imported records SHALL pass `check` under the format the project chose.

#### Scenario: Import {#S1}
- **WHEN** records from adr-tools and then from MADR and Log4brains are imported
- **THEN** each becomes a node with its status and date, a record that supersedes another links to it, a second import refuses to overwrite, and the project passes `check`

#### Scenario: A status with more words after it {#S2}
- **WHEN** a record says `Accepted (built)` or `Accepted, amended by ADR-0003`
- **THEN** its status is accepted, the rest is kept as a note, and an amended-by link becomes a link of the record that amends

#### Scenario: A project without the module {#S3}
- **WHEN** records are imported into a project whose `adrs` module is off
- **THEN** the module is switched on and the records are written
