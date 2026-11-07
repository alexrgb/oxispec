---
id: REQ-CLAIMS-0002
type: requirement
status: accepted
date: 2026-11-07
---
### Requirement: Claims are listed with the vocabulary
The system SHALL list the variables of the vocabulary with their types and the claims written against each, with the share of requirements that make a claim, as text and as JSON, and narrow the list to one variable.

#### Scenario: The list {#S1}
- **WHEN** requirements make claims about a duration and a role
- **THEN** each variable is listed with its type and claims, a variable with none says so, the JSON has the share of requirements that make a claim, and an unknown variable exits with 2
