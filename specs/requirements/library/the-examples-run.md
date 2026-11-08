---
id: REQ-LIBRARY-0001
type: requirement
status: accepted
date: 2026-11-08
---
### Requirement: The examples of the library run
The system SHALL ship examples of the library that build with it and run without error, and a test SHALL run each of them and read what it prints, so that an example that stops working fails the build.

#### Scenario: Every example {#S1}
- **WHEN** the examples read specs, check them, count coverage, pack context for an agent, compare two states and search an API
- **THEN** each one exits with 0 and prints the result the guide describes
