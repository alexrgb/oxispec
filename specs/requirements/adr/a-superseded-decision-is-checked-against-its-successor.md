---
id: REQ-ADR-0003
type: requirement
status: accepted
date: 2026-11-08
---
### Requirement: A superseded decision is checked against its successor
The system SHALL warn when a decision that a later decision supersedes does not say it was superseded, and SHALL fail on that warning with `--strict`.

#### Scenario: A successor {#S1}
- **WHEN** a decision supersedes another that is still marked accepted
- **THEN** `check` warns and passes, `check --strict` fails, and after the first is marked superseded it passes

```claims
for strict: warnings.fail
for not strict: not warnings.fail
```
