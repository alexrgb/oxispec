---
id: REQ-API-0003
type: requirement
status: accepted
date: 2026-11-06
---
### Requirement: An operation has a card
The system SHALL gather, for an operation named by its call or by its requirement, the product requirements document it refines, the decisions it was made by, the tech debt that arises from it, the risks that arise from it or that it mitigates, the test plans that cover it, and its scenarios.

#### Scenario: The card {#S1}
- **WHEN** a requirement states `GET /orders/{id}` and a PRD, a decision, a debt item, a risk and a test plan are linked to it
- **THEN** the card lists each of them and the scenarios of that operation, and a call that no requirement states selects nothing
