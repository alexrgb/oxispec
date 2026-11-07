---
id: REQ-SERVE-0002
type: requirement
status: accepted
date: 2026-11-07
---
### Requirement: The server refuses an unsafe start
The system SHALL refuse to start `oxispec serve` without a token, and on an address other than the local one unless it is told that a TLS proxy stands in front.

#### Scenario: Start {#S1}
- **WHEN** the server is started with no tokens, and then on 0.0.0.0 with a token
- **THEN** the first start fails with a message about no tokens, the second names `--behind-tls-proxy`
