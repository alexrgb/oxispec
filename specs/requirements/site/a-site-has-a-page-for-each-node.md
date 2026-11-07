---
id: REQ-SITE-0001
type: requirement
status: accepted
date: 2026-11-07
---
### Requirement: A site has a page for each node
The system SHALL build a static site with a page for each node, linked by ID, with the scenarios as anchors and the nodes that link to a node listed on its page, and SHALL refuse an output folder that is the project itself.

#### Scenario: The pages {#S1}
- **WHEN** a site is built from a requirement linked to a decision
- **THEN** each node has a page, a requirement has an anchor for each scenario, the decision page lists what links to it, and a node left out of the config is not a page

#### Scenario: The output folder {#S2}
- **WHEN** the output folder is the project folder
- **THEN** the command exits with 2 and says the site needs a folder of its own
