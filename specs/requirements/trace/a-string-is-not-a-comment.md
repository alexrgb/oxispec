---
id: REQ-TRACE-0003
type: requirement
status: accepted
date: 2026-11-06
---
### Requirement: A marker inside a string is not an annotation
The system SHALL NOT read a marker that stands inside a string literal on its line as an annotation, and SHALL still read one in a comment after code that has strings before it.

#### Scenario: Strings {#S1}
- **WHEN** a marker follows a comment leader inside a string, and another follows a comment after code with strings and a quote character
- **THEN** the first is not read and the second is
