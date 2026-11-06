---
id: REQ-RISK-0003
type: requirement
status: accepted
date: 2026-11-06
---
### Requirement: A risk is accepted by a person
The system SHALL report an accepted risk that names no person in `approved_by`, SHALL record the person when a risk is accepted with the `risks status` command, and SHALL warn about a live risk that has no owner, is past its review day, or is being mitigated by nothing.

#### Scenario: Accepting {#S1}
- **WHEN** a risk is accepted without a name in `approved_by`
- **THEN** the finding E039 is reported, and none once a person is named

#### Scenario: Held to account {#S2}
- **WHEN** a live risk has no owner, or is past its review day, or is mitigating with no `mitigated-by` link
- **THEN** W017, W018 or W019 is reported, and a closed risk raises none of them
