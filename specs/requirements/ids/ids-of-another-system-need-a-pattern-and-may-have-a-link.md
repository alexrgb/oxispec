---
id: REQ-IDS-0003
type: requirement
status: accepted
date: 2026-11-26
---
### Requirement: IDs of another system need a pattern and may have a link
The system SHALL let a project give `external:`, the IDs of another system such as Jira with a pattern each, SHALL treat a match as a known reference, and SHALL build a link to it only when the project gives a URL that starts with `http://` or `https://` and has `{id}` in it, and SHALL never fetch anything from that system.

#### Scenario: The link is optional {#S1}
- **WHEN** `external.KAN` has the pattern `KAN-[0-9]+`, with and without `url: https://example.atlassian.net/browse/{id}`
- **THEN** `KAN-7` is external in both, and has the link `https://example.atlassian.net/browse/KAN-7` only with the URL; a URL with no `{id}` or with another scheme is refused

#### Scenario: Trace shows the link {#S2}
- **WHEN** `oxispec trace KAN-7` is run and the project gives the URL
- **THEN** the output names the kind, the link and the places that mention the ID
