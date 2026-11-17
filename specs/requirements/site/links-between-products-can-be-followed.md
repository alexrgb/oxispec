---
id: REQ-SITE-0005
type: requirement
status: accepted
date: 2026-11-17
---
### Requirement: Links between products can be followed
The system SHALL, in a site of products, make a link to a node of another product a link to that product's page, and SHALL list on the page of a node the nodes of other products that link to it.

#### Scenario: Following a link {#S1}
- **WHEN** a product links to a node of the platform and the site is built
- **THEN** the page of the product links to the page of that node in the folder of the platform, and the page of the node lists the product's node under the products that use it
