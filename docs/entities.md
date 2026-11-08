# Entities

An entity is a thing the specs are about: an order, a user, a session, a release. Many requirements talk about the same entity, and when it changes, requirements in other parts of the system can stop being right. An entity node gives that thing one place to live, so a change to it can be followed to what depends on it.

It needs the `consistency` module (the same as claims, because the attributes of an entity are the variables claims talk about).

## Write one

```bash
oxispec new entity "Order"
```

This writes `specs/entities/0001-order.md`. Its `entity` block has a name and, if the thing has them, typed attributes:

````markdown
```entity
name: order
attributes:
  total: {type: int}
  status: {type: enum, values: [open, paid, shipped]}
```
````

The attribute types are those of the [vocabulary](claims.md#the-vocabulary): `int`, `duration`, `bool` and `enum`. An attribute is a variable named `ENTITY.ATTRIBUTE`, here `order.total` and `order.status`, so a requirement can claim something about it without a vocabulary entry of its own:

````markdown
```claims
order.total <= 1000
```
````

An entity with no attributes is fine. A name is letters, digits, `_` and `.`, and is used by one entity only.

## Say what is about it

A requirement, a test or a decision says it is about an entity with a link:

```bash
oxispec link REQ-ORDERS-0001 about ENT-0001
```

`oxispec check` warns (W024) when `about` points at something that is not an entity, and reports (E041) an entity block that cannot be read.

## See what depends on it

```bash
oxispec entities                 # each entity, its attributes, and how many nodes depend on it
oxispec entities ENT-0001        # one entity, and each node that depends on it
oxispec entities ENT-0001 --depth 3 --json
```

A node depends on an entity when it links to it, when one of its claims uses an attribute of it, or, up to `--depth` links away (2 by default), when it links to such a node. The nearest come first, and each says how it is connected: `about ENT-0001`, `claims order.total`, or `refines REQ-ORDERS-0001`. An agent asks the same with the MCP tool `spec_entities`.

## See what a change touches

`oxispec ci` compares the entities of a branch with those where it started, and for each one that changed it says what changed and what depends on it, in the output, in the JSON and in the pull request comment:

```
entity ENT-0001 (order) [breaking]: `status` now also allows returned; attribute `total` removed; 2 depend on it
```

| Change | Breaking? |
|---|---|
| An attribute is added | No |
| Values are added to an enum | No |
| An attribute is removed | Yes |
| An attribute changes its type | Yes |
| Values are removed from an enum | Yes |
| The entity is renamed or removed | Yes |

"Breaking" means a reader of the entity can stop working. It is a rule of thumb, not a proof. It is information and never a finding: a breaking change does not fail the run by itself, because only a person can say whether the nodes that depend on the entity are still right. What does fail is a claim that can no longer be read, such as one on an attribute that was removed (E017), which is how the dependents that matter most show up as errors too.

## What it does not do yet

- It does not follow a change into code. Only nodes are followed.
- It does not know what a requirement says about an entity unless a link or a claim does. A requirement that only mentions an order in its prose is not found.
- It does not mark the dependents as needing review until someone looks, the way `oxispec suspect` does for code. That is the next step.
- Attributes have the four types of the vocabulary. There are no structured types, lists or relations between entities beyond links.
