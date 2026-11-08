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

## Say which code represents it

Put the ID of the entity in a `spec:` comment above the struct, class, interface or type that represents it, the same comment that ties code to a requirement:

```rust
// spec: ENT-0001
pub struct Order {
    pub total: i64,
    pub status: Status,
}
```

`oxispec entities ENT-0001` then lists the code: where it is, its name and kind, and its fields.

```
  code  src/order.rs:2  Order (struct_item)  fields: total, status
```

Three things follow from the link, and none needs anything more written:

- **A field for each attribute.** `oxispec entities` and `oxispec ci` warn (W025) about an attribute that no field of that code matches. Case, `_` and `-` are ignored, so `total_amount`, `totalAmount` and `TotalAmount` are one name. Nothing is said when the comment is above a function, or above a type with no fields, since there is nothing to look in. Fields are read for Rust structs, Go structs, TypeScript interfaces, classes and object type aliases, and JavaScript classes. Other languages have no symbol, so the link is only a place.
- **Suspect links.** The link is watched like any other between a spec and code. When the struct changes, or the entity does, `oxispec suspect` lists it until someone looks and runs `oxispec confirm ENT-0001`. This is the review step for code: a change to the entity makes the code that represents it suspect.
- **The report of a change.** When a branch changes an entity, `oxispec ci` names the code that represents it, so whoever changes the entity sees what may need to follow.

More than one place can say it is about the same entity (a struct, a database row type, a DTO). An attribute only has to be a field of one of them, so a layer that carries part of the entity does not raise a warning. A function with `spec: ENT-0001` above it is listed as code, and is watched, but its fields are none.

## Look again when it changes

A node that is `about` an entity was written for the entity as it was. Say that you looked at it and it is still right:

```bash
oxispec confirm --all            # the first time, to record the starting point
oxispec confirm REQ-ORDERS-0001  # after looking at one node
```

From then on, when the entity gets a different name or attributes, each node about it is **to review**: `oxispec suspect` lists it and exits with 1, `oxispec entities ENT-0001` marks it `NEEDS REVIEW`, and `oxispec ci` warns (W026) in the output and the comment. Reading the node, changing it if it needs changing, and running `oxispec confirm NODE` settles it. The confirmation goes in `.oxispec/trace.lock.json`, which is committed, so a pull request that changes an entity and does not review what depends on it shows up, and one that does shows the confirmations in its diff.

Only what the entity says counts: its name, its attributes, their types and enum values. A reworded description does not make anything suspect. A node that was never confirmed is "unreviewed" and is not warned about, as with code: nothing is known about it yet. `oxispec suspect --strict` fails on those too.

W026 is a warning, so it fails `ci` only with `--strict`. `confirm ENT-0001` confirms the code that represents the entity (above), not the nodes about it: each node is for a person to look at.

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

- It does not read the types of the fields, so an attribute that is an `int` and a field that is a string are not told apart. Only names are compared.
- It does not follow a change into code beyond the places that say they are about the entity: a function that uses an `Order` and does not say so is not found.
- It does not know what a requirement says about an entity unless a link or a claim does. A requirement that only mentions an order in its prose is not found.
- A requirement that depends on an entity only through its claims, with no `about` link, is listed as depending on it but is not marked to review. Only `about` links are watched.
- A release is not held back by a node that needs review, the way it is by a suspect link to code.
- Attributes have the four types of the vocabulary. There are no structured types, lists or relations between entities beyond links.
