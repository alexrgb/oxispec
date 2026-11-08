---
id: REQ-FORMATS-0002
type: requirement
status: accepted
date: 2026-11-08
---
### Requirement: A decision picks its profile
The system SHALL write a decision in the profile the config or `--format` names, write the plain form when none is chosen, refuse a profile that belongs to another type, and report E031 in the config when it names a profile that does not exist.

#### Scenario: Choosing {#S1}
- **WHEN** `new adr` is run with no profile, with one, and with a profile of another type
- **THEN** the first writes the plain form, the second the chosen one, and the third exits with 2

#### Scenario: An unknown profile {#S2}
- **WHEN** the config names a profile that does not exist
- **THEN** `check` exits with 1 and reports E031 against `.oxispec/config.yaml`

```claims
exit.findings == 1
```
