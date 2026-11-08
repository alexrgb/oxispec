---
id: REQ-CI-0003
type: requirement
status: accepted
date: 2026-11-08
---
### Requirement: CI compares with where the branch started
The system SHALL compare a branch with the point where it left its base, not with newer work on the base, and SHALL explain how to get a base that is missing.

#### Scenario: Newer work on the base {#S1}
- **WHEN** the base gets a new requirement while the branch is open
- **THEN** the comparison shows only the branch's own requirement as added and nothing as removed

#### Scenario: A missing base {#S2}
- **WHEN** `ci` is run with no base, or in a repository without it
- **THEN** the command exits with 2 and says to use `origin/main` or to fetch with more depth
