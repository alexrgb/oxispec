---
id: REQ-RISK-0005
type: requirement
status: accepted
date: 2026-11-06
---
### Requirement: A critical risk holds a release back
The system SHALL treat a risk that is live and at the critical level as holding back a release, and SHALL NOT count a risk that is mitigated, accepted or closed, or one below the critical level.

#### Scenario: Critical and live {#S1}
- **WHEN** there are live and mitigated risks at several levels
- **THEN** only the live critical one is reported as holding the release back

#### Scenario: The gate {#S2}
- **WHEN** a release is checked and a live critical risk exists
- **THEN** the gate `no-critical-risks` fails and names it, passes once the risk is mitigated, and a project with no risks has no such gate
