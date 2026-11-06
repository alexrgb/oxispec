---
id: REQ-API-0005
type: requirement
status: accepted
date: 2026-11-06
---
### Requirement: A running service can be compared
The system SHALL compare the OpenAPI description a running service serves over plain HTTP with the requirements, exactly as it compares a file, and SHALL refuse an address that is not plain HTTP or that does not answer with a description.

#### Scenario: A service {#S1}
- **WHEN** a service answers a GET with a description, or answers with chunked encoding
- **THEN** its operations are compared with the requirements and the differences are reported

#### Scenario: Not usable {#S2}
- **WHEN** the address is HTTPS, or the service answers with an error status or text that is not a description
- **THEN** the command stops with the reason and exits with an error
