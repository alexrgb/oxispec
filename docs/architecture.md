# Architecture model

Hand-drawn diagrams go stale because each picture repeats the same boxes. OxiSpec keeps a **model**: you describe each element and each relationship once, and every view is generated from it. It needs the `architecture` module (`oxispec modules add architecture`, included in the `standard` preset).

Make elements with `oxispec new architecture system|person|container|component|environment "Name"`. Each is a node, written to `architecture/model/`, with one fenced `architecture` block of YAML:

````markdown
---
id: C4-0003
type: container
status: accepted
satisfies: [REQ-ORD-0003]
decided-by: [ADR-0007]
---
# C4-0003: Orders API

```architecture
name: Orders API
technology: Go, net/http
owner: team-orders
parent: C4-0001
uses:
  - {to: C4-0004, description: reads and writes orders, technology: SQL}
  - {to: C4-0006, description: charges cards, technology: HTTPS/JSON}
```
````

| Type | Is | `parent` |
|---|---|---|
| `person` | a user of the system | none |
| `system` | a software system, yours or external | none |
| `container` | something that runs: a web app, an API, a database, a queue | its system |
| `component` | a part of a container | its container |
| `deployment-environment` | a place things run, with `nodes` | none |

`satisfies`, `decided-by` and the other links are ordinary frontmatter links, so a requirement shows what delivers it. `uses` says what an element talks to. Write each relationship once, on the element that starts it. Code (C4 level 4) is not modelled; it stays in the code and links to specs through annotations.

An environment lists where containers and components run:

```architecture
name: Production
nodes:
  - {id: eks, kind: kubernetes-cluster, region: eu-west-1, hosts: [C4-0002, C4-0003]}
  - {id: rds, kind: managed-database, hosts: [C4-0004]}
```

## Views

A view is a query over the model.

```bash
oxispec architecture                                  # elements, environments and rules
oxispec architecture view context C4-0001             # a system, and the people and systems around it
oxispec architecture view container C4-0001           # the containers inside a system
oxispec architecture view component C4-0003           # the components inside a container
oxispec architecture view deployment DEP-0001         # an environment: nodes and what runs on them
oxispec architecture view container C4-0001 --json    # nodes, edges and the Mermaid text
```

The output is a Mermaid `flowchart`, which renders in GitHub, GitLab and most viewers. An element outside the view is shown at the level of the view: in a container view, a relationship from a component of another container shows as one from that container. Relationships that become the same pair of boxes merge into one arrow with the labels joined.

Mermaid has a C4 diagram type, but it is experimental, so views are flowcharts with C4 colours. PlantUML, Structurizr and D2 output are not done yet.

## Rules

A rule goes in the block of any element:

```architecture
rules:
  - forbid C4-0002 -> C4-0004      # the web app does not use the database directly
  - forbid C4-0002 ->* C4-0004     # nor through any chain of other elements
```

## Checks

`oxispec check` reports:

| Code | Meaning |
|---|---|
| E025 | a block cannot be read: bad YAML, an unknown key, a block in a node of the wrong type, a rule that is not `forbid A -> B` or `forbid A ->* B`, or two blocks in a node |
| E026 | a container or component has no `parent` or the wrong kind of one, or `uses`, `hosts` or a rule names something that is not an element, or an environment hosts a system or a person |
| E027 | a rule is broken. The message gives the path, for example `C4-0002 -> C4-0003 -> C4-0004` |
| W009 | a container has no owner (the `owner` field, or `owner:` in the frontmatter) or no technology |
| W010 | a container runs in no environment. Only reported once some environment exists |

Elements in a rejected, superseded, deprecated or withdrawn node are not checked and do not appear in views. Messages carry no line numbers, so `oxispec ci` reports a new problem once.

## Agents

The `oxispec-model-architecture` skill (see [agents.md](agents.md)) teaches an agent to model the system and to leave a broken rule for a person. The MCP server has a `spec_architecture` tool, see [mcp.md](mcp.md).

## Not done yet

- Dynamic views (numbered interactions for one scenario).
- Relationships that name an API operation, and checks against API specs.
- Drift checks against code scanners and infrastructure files (Terraform, Kubernetes, Compose).
- Rules over properties such as regions, and `require` rules.
- PlantUML, Structurizr DSL and D2 output, and a release diff of the architecture.
