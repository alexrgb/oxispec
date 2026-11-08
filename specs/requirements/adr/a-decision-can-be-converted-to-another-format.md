---
id: REQ-ADR-0001
type: requirement
status: accepted
date: 2026-11-08
---
### Requirement: A decision can be converted to another format
The system SHALL rewrite a decision in another ADR format with `adr convert`, move its sections, show what it would do with `--dry-run`, change nothing on a second run, and refuse a node that is not an ADR or a profile that does not exist.

#### Scenario: Convert {#S1}
- **WHEN** a decision is converted to MADR with `--dry-run` and then for real, and then again
- **THEN** the dry run writes nothing, the sections move under the new headings, and the second run changes nothing

#### Scenario: Refusal {#S2}
- **WHEN** `convert` is given a requirement or an unknown profile
- **THEN** the command exits with 2 and says it is not an ADR, or that no profile has that name
