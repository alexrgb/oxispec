---
id: REQ-PROJECT-0007
type: requirement
status: accepted
date: 2026-11-17
---
### Requirement: A project below a project is a product
The system SHALL treat a folder that has a project of its own as a product: the project above it SHALL leave its files out of its graph, `oxispec products` SHALL list the products, and `check` SHALL go through each product with the product's own config and name the product in the path of a finding.

#### Scenario: Products are left out and listed {#S1}
- **WHEN** a project has two products, one inside the other's folder tree, and a document of its own
- **THEN** `list` shows only the document of the project, and `products` lists the outer products with their node counts

#### Scenario: Each product is checked with its own config {#S2}
- **WHEN** two products use different formats and the same ID, the project above chooses no format, and then a product is broken
- **THEN** `check` of the project passes at first and counts the nodes of all the products, and then exits with 1 and names the product in the path of the finding
