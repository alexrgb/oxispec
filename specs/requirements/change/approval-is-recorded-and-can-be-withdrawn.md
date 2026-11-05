---
id: REQ-CHANGE-0002
type: requirement
status: accepted
date: 2026-11-05
---
### Requirement: Approval is recorded and can be withdrawn
The system SHALL report an approved or merged change set that has no approver, and SHALL withdraw an approval when the change set goes back to draft or in-review.

#### Scenario: No approver {#S1}
- **WHEN** a change set is approved or merged and `approved_by` is missing or blank
- **THEN** the finding E013 is reported

#### Scenario: Going back {#S2}
- **WHEN** a change set moves back to draft or in-review
- **THEN** its earlier approval is withdrawn, and a merge keeps it
