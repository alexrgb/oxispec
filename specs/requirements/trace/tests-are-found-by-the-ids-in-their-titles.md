---
id: REQ-TRACE-0007
type: requirement
status: accepted
date: 2026-11-26
---
### Requirement: Tests are found by the IDs in their titles
The system SHALL let a project give `trace.test_ids`, patterns for IDs that a test names in its title, and SHALL read each match as a `verifies` of the node with that ID, leave out a match that no node has, and refuse a pattern that is not valid.

#### Scenario: A match counts {#S1}
- **WHEN** the pattern `TC-[A-Z]+-[0-9]+` is set, a test is titled `TC-CAT-001 lists programs`, and a node `TC-CAT-001` exists
- **THEN** the test verifies that node, and `trace` on the node lists the test

#### Scenario: No node, or a bad pattern {#S2}
- **WHEN** a title matches the pattern but no node has that ID, or a pattern is not a valid regular expression
- **THEN** the match is left out without a finding, and the bad pattern stops the scan with the reason
