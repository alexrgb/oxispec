---
name: oxispec-model-state-machines
description: "Model a lifecycle as a state machine: states, events and transitions, plus properties such as never cancel after shipped, checked with a counterexample path when they fail. Use when a requirement describes states or an order of events, or when oxispec check reports a state machine problem."
metadata:
  generated-by: oxispec
  content-hash: "9cb1106feccd7494"
---

# Model a lifecycle as a state machine

A state machine is the part of a spec that says which states a thing can be in and which events move it. `oxispec check` reads it, finds mistakes in it, and checks properties that requirements state about it.

## Steps

1. Read the requirements that describe the lifecycle (the `oxispec-explore` skill). List the states and the events they name.
2. See what exists: `oxispec machine`. Edit a machine that is there, or create one with `oxispec new machine "Name"`.
3. Write a `statemachine` block in the node:

   ```statemachine
   initial: created
   states: [created, paid, shipped, delivered, cancelled]
   final: [delivered, cancelled]
   transitions:
     - {from: created, event: pay, to: paid, implements: [REQ-ORD-0003]}
     - {from: paid, event: ship, to: shipped}
     - {from: shipped, event: deliver, to: delivered}
     - {from: created, event: cancel, to: cancelled}
     - {from: paid, event: cancel, to: cancelled, guard: "not yet packed"}
   properties:
     - {rule: never cancel after shipped, from: REQ-ORD-0009}
   ```

   Names use letters, digits, `_`, `.` and `-`. `implements` and `from` name the requirements the transition or property comes from.
4. A property is one of:
   - `never reach S`: no run enters S;
   - `never E after S`: no run takes E once it has been in S;
   - `S leads to T`: every run that enters S enters T later;
   - `E requires S`: no run takes E unless it was in S before.
5. Run `oxispec check`, then `oxispec machine SM-ID`:
   - `E021` names a state that is not in `states`;
   - `E022` is two transitions on one event that can both apply. Give them guards that cannot both hold;
   - `E023` is a violated property, with the path that breaks it. Guards are not evaluated, so a guard may rule the path out in practice;
   - `W007` is a state nothing reaches, `W008` a state with no way out that is not final.
6. `oxispec machine SM-ID --tests` lists one test per transition, as a path from the initial state. `--diagram` prints a Mermaid diagram.

## Rules

- Model only what the requirements say. Do not add states or transitions to make a check pass.
- When a property fails, the machine or the requirement is wrong. Show a person the path. Do not delete the property.
- A guard is a short phrase for people. Two transitions on the same event need different guards.
