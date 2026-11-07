---
id: REQ-SITE-0002
type: requirement
status: accepted
date: 2026-11-07
---
### Requirement: A site can show the specs at a revision
The system SHALL build the site from the specs as they were at a git revision when asked with `--at`.

#### Scenario: An old revision {#S1}
- **WHEN** a site is built at an earlier revision and from the working tree
- **THEN** the old site has the nodes of that revision only, and the text comes from git
