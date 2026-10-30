# Claims and the vocabulary

A requirement is prose. A **claim** is the part a machine can check. It sits in a fenced `claims` block next to the text, and `oxispec check` uses it to find requirements that contradict each other. It needs the `consistency` module (`oxispec modules add consistency`, included in the `standard` preset).

````markdown
### Requirement: Admin session timeout
The system MUST expire admin sessions after 15 minutes of inactivity.

#### Scenario: Idle admin  {#S1}
- **WHEN** an admin is inactive for 15 minutes
- **THEN** the session is invalidated

```claims
for role = admin: session.idle_timeout <= 15m
```
````

Another requirement says `for *: session.idle_timeout == 30m`. `oxispec check` reports:

```
error[E019]: claims on session.idle_timeout conflict when role = admin: REQ-AUTH-0004 says "for *: session.idle_timeout == 30m", REQ-AUTH-0012 says "for role = admin: session.idle_timeout <= 15m". No value satisfies them all; add an `overrides` link, narrow a scope, or supersede one
```

For a user who is not an admin there is no conflict, because only one claim applies.

## The vocabulary

Claims talk about **variables**, and a vocabulary declares them, so two requirements mean the same thing by `session.idle_timeout`. It is a node of type `vocabulary` with a fenced `vocabulary` block of YAML. Make one with `oxispec new vocabulary "Auth words"`.

```vocabulary
session.idle_timeout: {type: duration}
password.min_length: {type: int}
role: {type: enum, values: [admin, user, guest]}
mfa: {type: bool}
```

| Type | Values | Written as |
|---|---|---|
| `int` | whole numbers | `12`, `-1` |
| `duration` | zero or more, kept in milliseconds | `500ms`, `30s`, `15m`, `2h`, `1d`, `1w` |
| `bool` | `true`, `false` | `mfa`, `not mfa`, `mfa = true` |
| `enum` | the listed `values` | `admin` |

A name is letters, digits, `_` and `.`, and is not a keyword. Several vocabulary nodes can exist, and a name declared twice is an error (E018). `oxispec claims` lists the variables and the claims on each.

The vocabulary is a node rather than a separate `vocab.yaml` so that it has an ID, history, links and a place in `oxispec diff` and `oxispec ci` like everything else. It is read at any revision with `--at`.

## Writing claims

One claim per line: `[must | must not | should | should not] [for SCOPE:] CONSTRAINT`. Keywords are not case sensitive. Blank lines and lines starting with `#` are skipped.

- **Scope** says when the claim applies: `for *:`, or no `for` at all, means always. Otherwise it is a test over other variables, such as `for role = admin and env = prod:`.
- **Constraint** is what must hold. It is about **one variable**, so the solver is exact. For two variables, write two lines.
- **Tests** are `==` (or `=`), `!=`, `<`, `<=`, `>`, `>=` and `x in [a, b]` (also `x not in [...]`). Join them with `and`, `or` and `not`, and use parentheses to group. Enums and bools compare with `=`, `!=` and `in` only.
- **`must not C`** means `must not-C`. **`should`** is a wish: it can only warn.

A scope cannot use the variable the claim constrains.

## What is reported

| Code | Level | Meaning |
|---|---|---|
| E017 | error | A claim cannot be read: bad syntax, an unknown variable or value, a missing unit, or a constraint on more than one variable. The message says which |
| E018 | error | A vocabulary block cannot be read, or declares a variable twice |
| E019 | error | Claims conflict: in some context where all their scopes hold, no value satisfies all their constraints |
| W005 | warning | A `should` claim cannot hold in a context where `must` claims apply |
| W006 | warning | A claim never applies, because its scope can never hold (`for role = admin and role = guest`) |
| I002 | info | A claim says nothing that another claim does not already say. Shown with `--verbose` |

A conflict names the **smallest** set of claims that cannot hold together, usually two, and the situation (`when role = admin`) where it happens. A claim that asks for no value at all (`session.idle_timeout < 0s`) is a conflict by itself.

Messages carry no line numbers, so `oxispec ci` does not report an old conflict as new when a file above it changes.

## When claims count

Claims in a node count unless its status is `rejected`, `superseded`, `deprecated` or `withdrawn`, or another node `supersedes` it. A `draft` or `proposed` node counts: a conflict is cheapest to fix when the change is reviewed. A claim that cannot be read is reported whatever the status.

## Settling a conflict

There are three ways, and a person chooses:

1. **Narrow a scope**, so the claims no longer meet.
2. **Say that one wins.** `oxispec link REQ-AUTH-0012 overrides REQ-AUTH-0004` makes REQ-AUTH-0012's claims win over REQ-AUTH-0004's wherever both apply. The loser's claims are left out there, so a conflict with a third claim still shows.
3. **Supersede** the old requirement.

## How it works

Two claims conflict when some context makes both scopes hold and no value satisfies both constraints. A claim that applies nowhere conflicts with nothing, so it is not enough to ask whether all constraints are satisfiable together.

For each variable, the checker takes the variables that its claims use in scopes, splits each into the **regions** that no scope can tell apart (each enum value that the scopes treat differently, each stretch of numbers between two thresholds), and looks at every combination. In each one it finds the claims that apply, drops the ones that are overridden, and intersects what they allow. Values are sets: intervals for numbers and durations, a finite set for enums and bools, so the answer is exact and needs no solver library. An empty intersection is a conflict, which is then cut down by removing claims one at a time while it stays empty.

If one variable's claims meet in more than 4096 combinations, the checker looks at pairs of claims whose constraints cannot hold together. That finds what two claims do, and misses a conflict that needs three or more claims and has too many scopes to list. The redundancy note (I002) is skipped for a variable with more than 100 claims.

Tested on 4,000 claims on one variable: a check takes about 1.5 seconds. A project without claims pays nothing.

## Not yet

- Claims about state machines, APIs and architecture rules (later iterations).
- Constraints over more than one variable, and permissions as `can(role, action)`.
- Version ranges, so claims that differ over time are not compared.
- Claims imported from another project.
- A baseline file for conflicts that existed before. `oxispec ci` already reports only new ones.
- Claims written by the agent from free text with a language model. The `oxispec-extract-claims` skill has the agent write them, and the checker decides.
