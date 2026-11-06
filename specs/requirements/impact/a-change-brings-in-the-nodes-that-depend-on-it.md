---
id: REQ-IMPACT-0001
type: requirement
status: accepted
date: 2026-11-06
---
### Requirement: A change brings in the nodes that depend on it
The system SHALL bring into the impact of a changed node the nodes that link to it, including through a scenario and through its old ID when it was deleted, and SHALL NOT bring in what a changed node links to, and SHALL keep the findings only of nodes in scope.

#### Scenario: Referrers {#S1}
- **WHEN** a node changes
- **THEN** the nodes that link to it are in scope

#### Scenario: Not the other way {#S2}
- **WHEN** a node that links to others changes
- **THEN** what it links to is not brought in

#### Scenario: A deleted node {#S3}
- **WHEN** a node is deleted
- **THEN** it is found through its old ID

#### Scenario: A scenario {#S4}
- **WHEN** a node links to a scenario of another
- **THEN** it counts as a link to that node

#### Scenario: Findings {#S5}
- **WHEN** findings exist inside and outside the scope
- **THEN** only those of nodes in scope are kept

#### Scenario: A duplicate ID {#S6}
- **WHEN** an ID is duplicated and the finding is reported on the other file
- **THEN** the finding is kept
