# State machines

A requirement says what a thing must do. A **state machine** says which states it can be in and which events move it, so a tool can check the lifecycle and not only read about it. It needs the `statemachines` module (`oxispec modules add statemachines`, included in the `standard` preset).

Make one with `oxispec new machine "Order"`. It is a node, usually of type `state-machine`, with one fenced `statemachine` block of YAML:

````markdown
---
id: SM-ORDER
type: state-machine
status: accepted
---
# SM-ORDER: Order lifecycle

```statemachine
initial: created
states: [created, paid, shipped, delivered, cancelled]
final: [delivered, cancelled]
transitions:
  - {from: created, event: pay,     to: paid,      implements: [REQ-ORD-0003]}
  - {from: paid,    event: ship,    to: shipped,   implements: [REQ-ORD-0005]}
  - {from: shipped, event: deliver, to: delivered}
  - {from: created, event: cancel,  to: cancelled}
  - {from: paid,    event: cancel,  to: cancelled, guard: "not yet packed"}
properties:
  - {rule: never cancel after shipped, from: REQ-ORD-0009}
```
````

Names of states and events use letters, digits, `_`, `.` and `-`. A node holds one machine, and the block can sit in any node. A machine in a rejected, superseded, deprecated or withdrawn node is not checked.

A **guard** is a phrase for people. It is not evaluated. `implements` and a property's `from` name the requirements the transition or the property comes from, and a name that does not exist is an error.

## Properties

A property is one line, in one of four forms:

| Form | Means |
|---|---|
| `never reach S` | no run enters S |
| `never E after S` | no run takes the event E once it has been in S |
| `S leads to T` | every run that enters S enters T later |
| `E requires S` | no run takes E unless it was in S before |

A **run** is a path from the initial state. It can stop in a final state, or in a state with no transition. Reachability treats every transition as possible, so a guard may rule out a path in practice. `S leads to T` assumes nothing about fairness: a loop that can repeat forever counts as a run that never arrives. A property about a state no run reaches holds, because there is nothing to break it.

A violated property comes with the shortest path that breaks it:

```
error[E023]: state machine SM-ORDER: the property `never cancel after paid` (from REQ-ORD-0009) is violated by created -pay-> paid -cancel-> cancelled
```

## Checks

`oxispec check` reports:

| Code | Meaning |
|---|---|
| E020 | the block cannot be read: bad YAML, an unknown key, a bad name, a state listed twice, a property that is not one of the four forms, or two blocks in a node |
| E021 | `initial`, `final`, a transition or a property names a state that is not in `states` |
| E022 | two transitions from one state on one event can both apply: one has no guard, or the guards are the same |
| E023 | a property is violated, with the path |
| E024 | `implements` or a property's `from` names a node that does not exist |
| W007 | a state cannot be reached from the initial state |
| W008 | a state has no way out and is not final |

Different guards on one event are taken to exclude each other. Messages carry no line numbers, so `oxispec ci` reports a new violation once and not again after it is merged.

## Looking at a machine

```bash
oxispec machine                      # every machine, with the number of violated properties
oxispec machine SM-ORDER             # transitions, and each property with its counterexample
oxispec machine SM-ORDER --diagram   # a Mermaid stateDiagram-v2
oxispec machine SM-ORDER --tests     # one test for each transition
oxispec machine SM-ORDER --json
```

`--at REV` reads the machine as it was at a git revision.

## Generated tests

`--tests` lists one test case per transition that can be reached. Each is the shortest path from the initial state, ending with that transition, and names the requirements the transition implements:

```
SM-ORDER: shipped on deliver to delivered
  created -pay-> paid -ship-> shipped -deliver-> delivered
  covers REQ-ORD-0005
```

This is a list for people and agents to turn into tests in your own framework. A longer path cover and export to XState or SCXML are not done yet.

## Agents

The `oxispec-model-state-machines` skill (see [agents.md](agents.md)) teaches an agent to write a machine from the requirements and to show a person any violated property without deleting it. The MCP server has a `spec_machine` tool, see [mcp.md](mcp.md).

## Limits

The block is read with a YAML parser that stops at 250,000 nodes, which is about 30,000 transitions. A larger block is reported as unreadable (E020). Split the machine, or model it at a coarser level.

## Not done yet

- Guards that are checked, and counters.
- Properties written in requirements and tied to a machine by ID, beyond the `from` field.
- Hierarchical and parallel states.
- XState and SCXML export, and a path cover for tests.
