---
id: REQ-RISK-0004
type: requirement
status: accepted
date: 2026-11-06
---
### Requirement: The register is ordered
The system SHALL list the risks that are live with the most serious level first and, within a level, the oldest first, and SHALL list the others only on request.

#### Scenario: Order {#S1}
- **WHEN** the register has risks of several levels and ages, and a closed one
- **THEN** the live ones come most serious first and oldest first, and the closed one comes last when all are asked for
