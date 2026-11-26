---
id: REQ-IDS-0001
type: requirement
status: accepted
date: 2026-11-26
---
### Requirement: IDs of the project's own shape are known
The system SHALL let a project give `ids:`, patterns for IDs it already uses (`TC: "TC-[A-Z]+-[0-9]+"`), and SHALL treat an ID that a whole pattern matches as a known reference even when no node has it: a link to it is not dangling, `trace` on it lists where it is named, and a pattern that is not a valid regular expression is refused with the key it is in.

#### Scenario: A pattern matches the whole ID {#S1}
- **WHEN** the pattern `TC-[A-Z]+-[0-9]+` is set
- **THEN** `TC-CAT-1200` is known, and neither `TC-CAT-1200x` nor `xTC-CAT-1200` is

#### Scenario: A link to a known ID is not dangling {#S2}
- **WHEN** a node links to `TC-CAT-1200`, a pattern matches it, and no node has that ID
- **THEN** `check` reports no `E002` for the link

#### Scenario: Trace lists where a known ID is named {#S3}
- **WHEN** `oxispec trace TC-CAT-1200` is run, a pattern matches the ID, no node has it, and a test names it in its title
- **THEN** the output says the ID has no node of its own and lists the test

#### Scenario: A bad pattern is refused {#S4}
- **WHEN** a pattern in `ids:` is not a valid regular expression
- **THEN** the config is not read, and the message names `ids` and the key
