---
id: REQ-FORMAT-PROFILES-0004
type: requirement
status: accepted
date: 2026-11-19
---
### Requirement: A record of work is written in a profile
The system SHALL write a PRD, a test plan, a risk and a tech debt item with the sections of the profile that `--format` or the config names, keep the fields of its type, refuse a profile that belongs to another type, and report E028 for a section the chosen profile requires and the node lacks.

#### Scenario: Choosing a profile for a record {#S1}
- **WHEN** `new prd`, `new test-plan`, `new risk` and `new debt` are run with a profile of their type, and `new risk` with a profile of another type
- **THEN** each node has the sections of its profile and the fields of its type, and the last command exits with 2

#### Scenario: A required section is missing {#S2}
- **WHEN** the config chooses the `one-pager` profile for PRDs and a PRD has no Problem section
- **THEN** `check` reports E028 for it

```claims
exit.findings == 1
```
