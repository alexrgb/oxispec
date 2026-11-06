---
id: REQ-TESTPLAN-0002
type: requirement
status: accepted
date: 2026-11-06
---
### Requirement: A plan is read against the tests
The system SHALL report, for each plan, the scenarios of the requirements it covers that no test verifies or that fail, and whether the requirements that mitigate each risk it covers are verified, and SHALL NOT count a plan as a test.

#### Scenario: The report {#S1}
- **WHEN** a plan covers a requirement with an unverified and a failing scenario, a risk mitigated by it, and a decision
- **THEN** the unverified and failing scenarios are listed, the risk is not tested, the decision is listed as other, and the plan has gaps
