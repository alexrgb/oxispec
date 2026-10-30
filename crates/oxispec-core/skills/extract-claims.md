# Write claims for a requirement

A claim is the part of a requirement that a machine can check. It sits in a `claims` block next to the text, and `oxispec check` uses it to find requirements that contradict each other.

## Steps

1. Read the requirement and what it links to (the `oxispec-explore` skill).
2. See which variables exist: `oxispec claims`. Use them. A new variable goes in a `vocabulary` block: edit the vocabulary node that is there, or create one with `oxispec new vocabulary "Name"`. Give it a type: `int`, `duration`, `bool`, or `enum` with its `values`.
3. Write one line per claim under the requirement:

   ```claims
   for role = admin: session.idle_timeout <= 15m
   ```

   The form is `[must | must not | should] [for SCOPE:] CONSTRAINT`. The scope says when the claim applies and may be `*` or left out. A scope and a constraint are tests joined with `and`, `or` and `not`, in parentheses where needed:
   - tests are `==` (or `=`), `!=`, `<`, `<=`, `>`, `>=`, and `x in [a, b]`;
   - a duration has a unit: `500ms`, `30s`, `15m`, `2h`, `1d`, `1w`;
   - a bool variable can stand alone, as `mfa` or `not mfa`.
4. A constraint is about one variable. Write one line for each variable. A scope may use any other variable.
5. Run `oxispec check`:
   - `E017` is a line that cannot be read, and the message says why;
   - `E019` is a conflict. It names the claims and the situation where they cannot both hold.
6. Show a person any conflict. Do not pick a side. A person decides to narrow a scope, to let one requirement win with `oxispec link REQ-A-0002 overrides REQ-A-0001`, or to supersede one.

## Rules

- Write only what the requirement says. Do not add limits the text does not state.
- `should` is for a wish, `must` for something that has to hold. A `should` that cannot hold only warns.
- Never delete a claim, or change a status, to clear a conflict.
- Claims in a rejected, superseded, deprecated or withdrawn node are not counted.
