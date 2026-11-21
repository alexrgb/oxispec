---
id: REQ-PROJECT-0009
type: requirement
status: accepted
date: 2026-11-21
---
### Requirement: The top project can read its products
The system SHALL let `list`, `read`, `search` and `show` run on one product with `--product`, let `list` and `search` include every product with `--all-products` (each node of a product named `product:ID`), let `show` and `read` find a node or a functionality that only one product has, and refuse, naming the products, when several have it.

#### Scenario: One product, or all of them {#S1}
- **WHEN** `list` and `search` are run at the top of a workspace with `--product` and with `--all-products`, and `--product` names a folder that is no product
- **THEN** the first shows the nodes of that product only, the second shows the nodes of every product named `product:ID`, and the third exits with 2 naming the products there are

#### Scenario: A node only one product has {#S2}
- **WHEN** `show` and `read` are run at the top for a node and a functionality of one product, and for a node that two products have
- **THEN** the first two read it from that product, and the last exits with 2 and names both products

```claims
exit.findings == 1
```
