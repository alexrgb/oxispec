# Compliance profiles

A compliance profile turns the traceability and sign-offs that safety standards ask for into rules that `oxispec check` and `oxispec release check` enforce. It helps a team collect the evidence. It does not make a project compliant: an assessor decides that, and some demands, such as independent review, cannot be seen in the specs.

## Turn it on

Add `compliance:` to `.oxispec/config.yaml`:

```yaml
compliance:
  standard: iec-62304
  level: B
```

`oxispec compliance standards` lists what is built in.

| Standard | Levels | Tier of each level |
|---|---|---|
| `iec-62304` (medical device software) | A, B, C | 1, 2, 3 |
| `iso-26262` (road vehicles) | A, B, C, D | 1, 1, 2, 3 |
| `do-178c` (airborne software) | E, D, C, B, A | 0, 1, 1, 2, 3 |

The tiers are cumulative. They are a starting point drawn from what the three standards have in common, not a copy of any of them; tune them with your own rules.

| Tier | Rule | Asks for |
|---|---|---|
| 1 | `requirement-verified` | A `verifies` link from a `test-case` node to the requirement or one of its scenarios |
| 1 | `requirement-signed-off` | `approved_by` and `approved_on` (a date) on the requirement |
| 2 | `requirement-allocated` | A `satisfies` link from a `component` |
| 2 | `adr-signed-off` | Sign-off on each decision record |
| 3 | `test-signed-off` | Sign-off on each test case |

A rule applies to nodes whose status is past draft: not `draft`, `proposed`, `rejected`, `deprecated`, `superseded`, `withdrawn` or `obsolete`. A node with no status is left alone.

## Your own rules

```yaml
compliance:
  rules:
    - id: risk-controlled
      type: requirement
      status: [accepted]        # optional; default: every status past draft
      needs: {link: decided-by, to: adr}
    - id: design-reviewed
      type: adr
      signoff: true
```

A rule has an `id`, a node `type`, and one of `needs` or `signoff`. `needs` names a link `kind` and either `from` (a node of that type links to this one) or `to` (this node links to a node of that type). Own rules are added to those of the standard; an id may not repeat.

## What it reports

| Code | Meaning |
|---|---|
| E035 | A node lacks a link a rule asks for |
| E036 | A node lacks the sign-off a rule asks for |
| E037 | The `compliance:` settings are not valid (unknown standard or level, a bad rule) |

`oxispec compliance` prints every rule with how many nodes meet it and lists the gaps. It exits 1 when there is a gap, so it can sit in CI. `--json` gives the same for a script.

`oxispec release check` has a blocking gate, `compliance-met`, when the project has rules. The tests, results and trace annotations in the code are not part of it: a rule sees the graph of nodes, so a `verifies` link needs a `test-case` node.
