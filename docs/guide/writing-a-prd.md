# Writing a product requirements document

A PRD in OxiSpec is not a file you write and import. It is a `prd` node for the purpose, plus the requirements that refine it. The document people read is made from them, so it cannot drift.

## Start one

```bash
oxispec new prd "Checkout"                      # specs/prd/0001-checkout.md, id PRD-0001
oxispec new requirement pay "Pay by card"
oxispec link REQ-PAY-0001 refines PRD-0001
```

The `prd` node has the sections Overview, Users, Goals, Non-goals, Success metrics and Open questions. Write them as plain text or lists. Each requirement has its statement and its scenarios, which are the acceptance criteria.

With an AI agent, ask it to write the PRD. The skill `oxispec-write-prd` asks you questions, shows a draft in the chat, and writes the nodes only when you agree. It leaves unknowns under Open questions and does not invent metrics or dates.

## Read it as a document

```bash
oxispec docs prd PRD-0001                       # print it
oxispec docs prd PRD-0001 --out checkout-prd.md # write a copy to share
```

The document has the node's sections, then a Functional requirements section with each refining requirement, its status and its scenarios as WHEN and THEN lines. A section with no text says it is not written yet. Do not edit the copy. Change the nodes and make it again.

## Keep it true

`oxispec check` sees the PRD like any node: a link to a node that does not exist is an error. Requirements still get `// spec:` and `// verifies:` comments in code and tests, so `oxispec coverage` shows which acceptance criteria have a test. When a requirement changes, `oxispec diff` and the suspect links show it, and the PRD text you make next time carries the change.

A PRD that already exists as a markdown file can be brought in once with `oxispec import specs --from prd --dir <file>`. See [Moving from another tool](importing.md).
