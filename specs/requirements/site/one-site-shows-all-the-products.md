---
id: REQ-SITE-0004
type: requirement
status: accepted
date: 2026-11-17
---
### Requirement: One site shows all the products
The system SHALL build, for a project with products, one site with a folder for each product built from the product's own project, a switcher between products on every page and a page at the top that lists them, and SHALL refuse `--at` and `--releases` for it.

#### Scenario: A folder for each product {#S1}
- **WHEN** `site` is run in a project with two products
- **THEN** each product has its pages in a folder of its own, its pages carry a product switcher, and the page at the top links to every product

#### Scenario: Revisions are refused {#S2}
- **WHEN** `site --releases` is run in a project with products
- **THEN** it exits with 2 and says to leave out `--at` and `--releases`
