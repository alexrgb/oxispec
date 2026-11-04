# Tech debt

Tech debt is a backlog of shortcuts a team took on purpose, kept as nodes of type `debt` in `specs/debt/`. It needs the `debt` module (`oxispec modules add debt`, included in the `standard` preset). An item is a node and not a comment in the code, so it has an owner, can be queried and linked, and a release can be held to a limit on it.

## An item

```markdown
---
id: DEBT-0003
type: debt
status: open
severity: high
owner: payments-team
date: 2026-11-04
due: 2027-01-31
cost: 3 days
interest: one failed deploy a month
links:
  arises-from: REQ-API-0003
---
# DEBT-0003: Retry loop has no backoff
```

| Field | Meaning |
|---|---|
| `status` | `open`, `planned`, `in-progress`, `resolved` or `accepted` (the risk is taken on purpose). The first three are *live*: still to be paid. |
| `severity` | `low`, `medium`, `high` or `critical`. |
| `owner` | A person or a team. |
| `date` | When the item was opened or last reviewed. `oxispec debt status` sets it to today. |
| `due` | The day by which it must be dealt with. Optional. |
| `cost`, `interest` | Free text: what paying it off takes, and what it costs while it stays. |
| `arises-from` (link) | The requirement, decision or component the shortcut is a compromise of. |

`oxispec new debt "Retry loop has no backoff" --severity high --from REQ-API-0003` writes the file, with `owner` and `due` empty for you to fill in.

## The backlog

`oxispec debt` lists what is still to be paid, the most serious first and, within a severity, the oldest first. Overdue and stale items are flagged. `--status`, `--severity` and `--all` (adds resolved and accepted items) narrow or widen it, and `--json` gives the same for scripts. The MCP tool `spec_debt` returns the same JSON.

`oxispec debt status DEBT-0003 in-progress` moves an item and records today as the day it was reviewed.

## Checks

| Code | Meaning |
|---|---|
| E032 | The status or severity is missing or not valid, or `date` or `due` is not a date as `YYYY-MM-DD`. |
| W013 | A live item has no owner. |
| W014 | A live item is past its `due` date. |
| W015 | A live item was last reviewed more than `debt.stale_days` ago (default 180). |
| W016 | A live item has no `arises-from` link. |

Items that are `resolved` or `accepted` raise none of W013 to W016. The checks use today's date, so a project can start to show W014 and W015 without anyone editing a file. They are part of `check`, so `oxispec ci` reports them like any other finding, and `--strict` makes the warnings fail.

## Settings

In `.oxispec/config.yaml`:

```yaml
debt:
  stale_days: 90        # default 180
  limits:               # the most live items a release may have, by severity
    critical: 0
    high: 3
```

With `limits`, `oxispec release check` has a blocking gate, `debt-within-limits`, that fails when a severity has more live items than its limit. A severity that is not listed has no limit. Moving an item to `accepted` or `resolved` takes it out of the count, so an item should be accepted only when a person decided to live with it.

## Limits

- An item links to nodes. Linking to a code symbol comes with the trace annotations: put `spec: DEBT-0003` in a comment, as for any node.
- The limits are counted over the whole project. There is no per-team or per-module limit yet.
- `check --at REVISION` judges old revisions by today's date.
