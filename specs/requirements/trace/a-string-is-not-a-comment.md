---
id: REQ-TRACE-0003
type: requirement
status: accepted
date: 2026-11-06
---
### Requirement: Only a real comment is an annotation
The system SHALL read a marker in a Rust, Go, TypeScript or JavaScript file only when it is the first thing in a real comment, so that a marker in a string, a raw string, an example in a doc comment or an index such as `s["REQ-A-0001.S1"]` is not an annotation, and SHALL read a scenario ID in brackets as a test name only outside Rust and outside comments.

#### Scenario: Strings and examples {#S1}
- **WHEN** a marker stands in a string, a raw string, a string over several lines, or as an example inside a doc comment
- **THEN** it is not read, and a marker in a comment after code, in a block comment, or after a string that holds a quote character is read
