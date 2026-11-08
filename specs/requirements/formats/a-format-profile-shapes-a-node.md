---
id: REQ-FORMATS-0001
type: requirement
status: accepted
date: 2026-11-08
---
### Requirement: A format profile shapes a node
The system SHALL write the template of a profile with `new`, report with the codes E028 and E029 a node that lacks a required section or field, and describe the profiles with `formats list` and `formats show`.

#### Scenario: A profile in force {#S1}
- **WHEN** a project chooses a profile and a node is written from it, then a required field is empty, then a section is removed
- **THEN** the empty field is E029, the missing section is E028 and names the node, and a complete node passes

#### Scenario: The profiles {#S2}
- **WHEN** `formats list` and `formats show` are run
- **THEN** they name the profiles, mark the ones chosen, give JSON, show a profile's statuses and sections, and exit with 2 for an unknown name
