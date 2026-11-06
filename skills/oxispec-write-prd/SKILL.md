---
name: oxispec-write-prd
description: "Write a product requirements document as nodes: interview the person, draft the purpose, goals and non-goals, then the requirements with their acceptance scenarios, and show the document before anything is written. Use when asked to write or start a PRD, or to turn an idea into specs."
metadata:
  generated-by: oxispec
  content-hash: "ac8608ce40fa3a46"
---

# Write a product requirements document

Turn an idea into a PRD that lives as nodes: one `prd` node for the purpose, and requirements that refine it. Nothing is imported afterwards, and nothing is written before the person has seen the draft and said yes.

## Steps

1. Search first: `oxispec search "<topic>"`. If a PRD or requirements for it exist, extend them instead of starting another.
2. Ask, a few questions at a time: who the users are, what problem they have, what success looks like, what is out of scope, what is still unknown. Do not invent answers. Where the person does not know, the answer goes under Open questions.
3. Show a draft in the chat: the overview, users, goals, non-goals, success metrics and open questions, then the list of requirements, each as a one-line SHALL statement with its acceptance scenarios. Let the person change it.
4. When they agree, write it:
   - `oxispec new prd "<title>"`, then fill in its sections.
   - For each requirement: `oxispec new requirement <domain> "<title>"`, fill in the statement and scenarios, then `oxispec link <REQ-ID> refines <PRD-ID>`.
5. Run `oxispec check` and fix what it reports.
6. Show the document the nodes make: `oxispec docs prd <PRD-ID>`. Write it to a file with `--out` only if the person wants a copy to share.

## Rules

- The document is a view of the nodes. Change the nodes, never the generated file.
- Every requirement states one behavior and has at least one scenario with a WHEN and a THEN. Acceptance criteria are scenarios, not a separate list.
- Keep the status `draft` until a person approves the PRD.
- Do not write numbers for success metrics, deadlines or users that the person did not give you.
