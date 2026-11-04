---
name: oxispec-manage-debt
description: "Record a tech debt item, keep the backlog honest, and pay items off: owner, severity, due date, what it arises from. Use when you take a shortcut on purpose, find one, or are asked what debt there is."
metadata:
  generated-by: oxispec
  content-hash: "4690aed567eb3097"
---

# Manage tech debt

Keep the shortcuts a project takes on purpose as items that someone owns, instead of comments that nobody reads.

## Record a shortcut

1. Look for an existing item first: `oxispec debt --all`. Update that one instead of writing a second.
2. Create it: `oxispec new debt "Retry loop has no backoff" --severity medium --from REQ-API-0003`. `--from` is the requirement, decision or component the shortcut is a compromise of.
3. Fill in the three sections (what it is, why it was taken, what paying it off takes), `cost` (what paying it off takes) and `interest` (what it costs while it stays).
4. Set `owner` to a person or a team, and `due` when there is a date by which it must be dealt with.
5. Run `oxispec check`.

## Work the backlog

- `oxispec debt` lists what is still to be paid, the most serious and the oldest first. `--status`, `--severity` and `--all` narrow or widen it.
- `oxispec debt status DEBT-0003 in-progress` moves an item and records today as the day it was reviewed. Statuses are open, planned, in-progress, resolved and accepted.
- Mark an item `accepted` only when a person decided to live with it, and say why in the item. `resolved` means it was paid off.

## Rules

- `check` reports a live item with no owner (W013), one past its due date (W014), one not reviewed for longer than `debt.stale_days` (W015) and one with no `arises-from` link (W016). Fix the item; do not silence the warning.
- Never mark an item resolved to make a count pass. A release can be held to limits per severity under `debt.limits` in the config, and an honest count is the point.
- Do not invent a severity. If you are unsure, ask the person which of low, medium, high or critical it is.
