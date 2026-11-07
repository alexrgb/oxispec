---
id: REQ-SITE-0003
type: requirement
status: accepted
date: 2026-11-07
---
### Requirement: A site can hold every release
The system SHALL build, with `--releases`, one site for each tagged version and one for the latest specs, with a picker between versions and a root page that goes to the latest.

#### Scenario: Versions {#S1}
- **WHEN** two versions are tagged and a decision is added after them
- **THEN** each version and `latest` has its own pages and style, a version has only the nodes it had, the picker lists the versions, and the root page goes to `latest`
