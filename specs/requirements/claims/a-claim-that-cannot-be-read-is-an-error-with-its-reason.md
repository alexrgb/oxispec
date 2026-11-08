---
id: REQ-CLAIMS-0003
type: requirement
status: accepted
date: 2026-11-08
---
### Requirement: A claim that cannot be read is an error with its reason
The system SHALL report with E017 a claim that cannot be read, with the reason and the place in the file.

#### Scenario: A loose claim {#S1}
- **WHEN** a claim has no unit
- **THEN** `check` exits with 1 and reports E017, says what to write, and names the file and line

```claims
exit.findings == 1
```
