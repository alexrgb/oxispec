---
id: REQ-CI-0002
type: requirement
status: accepted
date: 2026-11-08
---
### Requirement: CI writes reports for the tools around it
The system SHALL write the findings of a change as SARIF, as JUnit XML and as a pull request comment that begins with a marker, so that CI can show and update it.

#### Scenario: Reports {#S1}
- **WHEN** a change adds an error and the three reports are asked for
- **THEN** the SARIF is version 2.1.0 with the rule and place, the JUnit has a failure for the node, and the comment begins with `<!-- oxispec-ci -->`
