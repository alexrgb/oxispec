---
id: REQ-ARCHITECTURE-0002
type: requirement
status: accepted
date: 2026-11-07
---
### Requirement: Architecture views come from one model
The system SHALL list the elements and the environments of the model, and draw the context, container, component and deployment views of it as Mermaid from the same elements.

#### Scenario: Views {#S1}
- **WHEN** a model of a system, containers, components and an environment
- **THEN** the list and the JSON agree, each view shows the elements that belong to it, and the deployment view shows where each container runs
