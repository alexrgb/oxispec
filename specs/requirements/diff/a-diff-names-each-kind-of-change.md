---
id: REQ-DIFF-0001
type: requirement
status: accepted
date: 2026-11-06
---
### Requirement: A diff names each kind of change
The system SHALL report the differences between two versions of a node as status, owner, move, link, requirement, scenario and text changes in a fixed order, SHALL report nothing for identical nodes or for a changed comment in the front matter, and SHALL report a removed requirement or scenario.

#### Scenario: Identical {#S1}
- **WHEN** two nodes are identical
- **THEN** there are no changes

#### Scenario: Every kind {#S2}
- **WHEN** a node changes in status, owner, place, links, requirements, scenarios and text
- **THEN** the changes are listed in one fixed order, each described

#### Scenario: Removals {#S3}
- **WHEN** a requirement or a scenario is removed
- **THEN** it is reported as removed

#### Scenario: A comment {#S4}
- **WHEN** only a comment in the front matter changes
- **THEN** there is no change
