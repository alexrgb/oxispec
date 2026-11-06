---
id: REQ-TRACE-0001
type: requirement
status: accepted
date: 2026-11-06
---
### Requirement: Annotations are read from comments
The system SHALL read `spec:` and `verifies:` annotations, with the IDs after them, from the comments of source files in many languages, and SHALL NOT read prose, a word that only ends in the marker, or a URL as an annotation.

#### Scenario: Comments {#S1}
- **WHEN** comments in Rust, hash-comment and SQL-style files hold the markers
- **THEN** each ID is read with its kind, line and file

#### Scenario: Lookalikes {#S2}
- **WHEN** a line has a marker in prose, a lower-case word after it, a word that ends in the marker, or an ID inside a URL
- **THEN** nothing is read
