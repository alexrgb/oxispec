---
id: REQ-RISK-0006
type: requirement
status: accepted
date: 2026-11-08
---
### Requirement: The register is an MCP tool when the module is on
The system SHALL offer the risk register as the MCP tool `spec_risks` only when the risks module is on, and refuse a level that does not exist.

#### Scenario: The tool {#S1}
- **WHEN** the module is off, then on with a critical risk, and a bad level is asked for
- **THEN** the tool is listed only with the module, it returns the risk, and a bad level is an error
