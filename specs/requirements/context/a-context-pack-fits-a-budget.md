---
id: REQ-CONTEXT-0002
type: requirement
status: accepted
date: 2026-11-06
---
### Requirement: A context pack fits a budget
The system SHALL fill a context pack with the nearest nodes first and the requirement before the decision at an equal distance, SHALL turn what does not fit into a pointer, SHALL cut an oversized node that was asked for rather than drop it, SHALL share one budget between several nodes and skip an unknown one, and SHALL count a token as about four characters.

#### Scenario: Order {#S1}
- **WHEN** nodes are at different distances, and a requirement and a decision at the same distance
- **THEN** the nearest come first and the requirement before the decision

#### Scenario: A pointer {#S2}
- **WHEN** a node does not fit in the budget
- **THEN** it becomes a pointer to the node

#### Scenario: An oversized node {#S3}
- **WHEN** the node asked for is bigger than the budget
- **THEN** it is cut, not dropped

#### Scenario: Several nodes {#S4}
- **WHEN** several nodes and an unknown ID are asked for
- **THEN** they share one budget and the unknown one is skipped

#### Scenario: Tokens {#S5}
- **WHEN** a text is counted
- **THEN** a token is about four characters
