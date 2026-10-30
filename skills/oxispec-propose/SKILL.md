---
name: oxispec-propose
description: "Propose a change to the specs as a change set, with new or edited requirements and scenarios, linked and checked, ready for a person to review. Use when behavior should change or a new requirement is needed."
metadata:
  generated-by: oxispec
  content-hash: "36b78cbfe32c3388"
---

# Propose a change to the specs

Write a change as a change set, so a person can review it before the specs change. You propose. A person approves.

## Steps

1. Explore first (the `oxispec-explore` skill). Look for an existing requirement to change before you add a new one.
2. Create the change set: `oxispec new change "Short title"`. It writes `specs/changes/NNNN-title.md`. Fill in why, what changes and the impact.
3. Edit or add the nodes the change touches:
   - a new requirement: `oxispec new requirement <domain> "Title"`, then write the statement and its scenarios in the file;
   - an existing requirement: edit the file, and raise `version` in the frontmatter when the meaning changes.
4. Give every scenario an anchor, such as `#### Scenario: Idle admin {#S1}`, so tests can name it.
5. Connect the nodes: `oxispec link CHG-0001 changes REQ-AUTH-0001`, and `oxispec link REQ-AUTH-0001 decided-by ADR-0001` where a decision explains it.
6. Run `oxispec check`. Fix every error. Read the warnings.
7. Move the change set to review: `oxispec change status CHG-0001 in-review`.

## Rules

- Do not run `oxispec change status ... approved`. Say that the change set is ready for a person to approve.
- Do not edit `approved_by` or `approved_on` by hand.
- Write each requirement with SHALL or MUST for something that has to hold, and keep to one behavior per requirement.
- Do not change a node with status `accepted` without a change set that says so.
