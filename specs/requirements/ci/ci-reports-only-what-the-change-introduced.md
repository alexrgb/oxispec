---
id: REQ-CI-0001
type: requirement
status: accepted
date: 2026-11-08
---
### Requirement: CI reports only what the change introduced
The system SHALL report, with `oxispec ci`, only the errors and warnings a change introduces against its base, fail on a new error, fail on a new warning only with `--strict`, and say how many findings were there already.

#### Scenario: Old and new {#S1}
- **WHEN** a project has an old error and a clean change, and then a change that adds an error
- **THEN** the clean change passes, the other fails and reports only its own error and counts the old one as existing

#### Scenario: Warnings {#S2}
- **WHEN** a change adds only a warning
- **THEN** the run passes and says there is a new warning, and with `--strict` it fails

```claims
for strict: warnings.fail
for not strict: not warnings.fail
```
