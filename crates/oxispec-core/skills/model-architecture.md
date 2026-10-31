# Model the architecture

Describe each element once. Every view (context, container, component, deployment) is generated from the model, so none of them goes stale on its own.

## Steps

1. Read the requirements and decisions that shape the system (the `oxispec-explore` skill). See what exists: `oxispec architecture`.
2. Create what is missing: `oxispec new architecture system "Shop"`, and likewise `person`, `container`, `component` and `environment`. Each is a node with an `architecture` block:

   ```architecture
   name: Orders API
   technology: Go, net/http
   owner: team-orders
   parent: C4-0001
   uses:
     - {to: C4-0003, description: reads and writes orders, technology: SQL}
   ```

   - A container's `parent` is its system, a component's is its container.
   - `uses` says what an element talks to. Write each relationship once, on the element that starts it.
   - An environment lists where things run: `nodes: [{id: eks, kind: kubernetes-cluster, region: eu-west-1, hosts: [C4-0002]}]`.
3. Tie elements to the spec with ordinary links in the frontmatter: `satisfies: [REQ-ORD-0003]`, `decided-by: [ADR-0007]`.
4. A rule goes in the block of any element: `forbid C4-0002 -> C4-0004` forbids a direct use, `forbid C4-0002 ->* C4-0004` any use through other elements. Write a rule only when a requirement or decision states it.
5. Run `oxispec check`:
   - `E025` a block that cannot be read;
   - `E026` a `parent`, `uses`, `hosts` or rule that names the wrong thing;
   - `E027` a broken rule, with the path that breaks it;
   - `W009` a container with no owner or technology, `W010` a container that runs nowhere.
6. Draw it: `oxispec architecture view container C4-0001`. The kinds are `context` and `container` (of a system), `component` (of a container) and `deployment` (of an environment). The output is Mermaid.

## Rules

- Model what is built or decided. Do not invent elements to make a view look complete.
- When a rule is broken, the model or the rule is wrong. Show a person the path. Do not delete the rule.
- Do not draw diagrams by hand. Change the model and draw again.
