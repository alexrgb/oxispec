---
id: REQ-SERVE-0003
type: requirement
status: accepted
date: 2026-11-07
---
### Requirement: Every request needs a token and is audited
The system SHALL answer a request to the protocol only with a valid bearer token, stop honouring a token once it is revoked, and write each request to an audit log with the principal, the tool, the node and the outcome, without the secret or the words searched.

#### Scenario: The protocol {#S1}
- **WHEN** a client with no token, a wrong token, and a good token ask the server
- **THEN** the health address answers anyone, the protocol answers 401 to the first two, and the good one is answered

#### Scenario: The audit log {#S2}
- **WHEN** tools are called by two tokens, one is denied, and one token is revoked
- **THEN** the log has an entry for each call with its outcome (ok, denied, unauthenticated), no secret and no search words, and the revoked token gets 401
