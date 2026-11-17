---
id: REQ-PROJECT-0008
type: requirement
status: accepted
date: 2026-11-17
---
### Requirement: A link can name a node of another product
The system SHALL read a link written as `product:ID`, where `product` is the folder of another project below the same workspace, check it in that project (E002 for a node that does not exist, E009 for a scenario that does not), leave a namespace that is no project as the unchecked reference I001, and show the nodes of other products that link to a node in `show` and in `ci`.

#### Scenario: Links are checked in the other product {#S1}
- **WHEN** a product links to a node of a platform project with `platform:REQ-AUTH-0001`, and then the link names a node that does not exist
- **THEN** `check` passes in the product and at the top at first, then reports E002 in both, and a namespace that is no project stays an I001

#### Scenario: A platform sees who depends on it {#S2}
- **WHEN** `show` is run for the linked node of the platform, and `ci` is run after the node was edited
- **THEN** both name the node of the product that links to it, and `ci` lists it in its JSON as a dependent
