# Write an architecture decision record

Record a decision that is hard to reverse or that a future reader will ask about.

## Steps

1. Search for an existing decision: `oxispec search "<topic>" --type adr`. If one covers it, link to it instead of writing another. If the new one replaces it, use `supersedes`.
2. Create the record: `oxispec new adr "Use Postgres for sessions"`. Fill in the context, the options considered, the decision and the consequences.
3. Keep the status `proposed` until a person accepts it.
4. Link it from what it explains: `oxispec link REQ-AUTH-0001 decided-by ADR-0001`. To replace an older record: `oxispec link ADR-0002 supersedes ADR-0001`.
5. Run `oxispec check`.

## Rules

- Write the options you rejected and why. A record with one option is not a decision.
- Never change an accepted record to reflect a new decision. Write a new record that supersedes it.
