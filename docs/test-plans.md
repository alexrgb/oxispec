# Test plans

A test plan says what is tested, how and where, and when testing is done. It is a node of type `test-plan` in `specs/test-plans/`, written with `oxispec new test-plan "Checkout release 1.2"`. It needs the `tests` module.

The plan is text for people (scope, approach, levels, environments, exit criteria) plus links that a program can read:

```markdown
---
id: TP-0001
type: test-plan
status: approved
owner: qa-team
date: 2026-11-06
links:
  covers: [REQ-PAY-0001, REQ-PAY-0002, RISK-0003]
---
# TP-0001: Checkout release 1.2
```

| Field | Meaning |
|---|---|
| `status` | `draft`, `approved` or `retired`. A person sets `approved`. |
| `covers` (link) | The requirements and risks in scope. |

A plan uses `covers` and not `verifies` on purpose. Only a test with `// verifies: REQ-PAY-0001.S1` verifies a scenario. A plan that linked to a requirement would otherwise make it look tested.

## Reading a plan against the tests

```bash
oxispec test-plan              # every plan
oxispec test-plan TP-0001      # one
oxispec test-plan --strict     # exit 1 when an approved plan has a gap
oxispec test-plan --json
```

For each requirement it covers, the report gives the number of scenarios, the ones no test names, and the ones whose imported results fail (`oxispec results import`). For each risk it covers, it says whether what mitigates it (`mitigated-by`) is tested: every scenario of those requirements is verified. A risk with nothing that mitigates it is reported as such. Anything else a plan covers (a decision, say) is listed and not read.

## Checks

| Code | Meaning |
|---|---|
| E040 | The status is missing or not valid, or `date` is not a date as `YYYY-MM-DD`. |
| W020 | A plan covers nothing. |
| W021 | A requirement is `accepted` and no approved plan covers it. |
| W022 | A live risk at the high or critical level is covered by no approved plan. |

W021 and W022 appear only in a project that has an approved plan, so a project that does not use plans sees neither.

## With an AI agent

The skill `oxispec-write-test-plan` writes a plan with the person, links what it covers, reads it against the tests, and leaves approval to a person.

## What is not built

A plan is not a release gate yet, and plans have no levels as data: the levels are text. Test cases as nodes (the `test-case` type) are not connected to plans.
