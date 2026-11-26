---
id: ADR-0005
type: adr
status: proposed
date: 2026-11-26
links:
  amends: [ADR-0001]
---
# ADR-0005: Nodes that live inside a document

## Context

A node is a file ([ADR-0001](0001-markdown-files-with-front-matter-are-the-source-of-truth.md)), and its ID is in front matter ([ADR-0002](0002-ids-are-immutable-and-independent-of-file-paths.md)). The only thing smaller than a file that has an ID is a scenario, which is a child of its requirement: `REQ-AUTH-0012.S1`.

A trial on a learning-platform project showed one whose documents were written before oxispec, in the usual way of a product team: one `spec.md` and one `test-cases.md` per epic, with a user story as a heading and a test case as a row of a table. Its IDs (`US-CAT-001`, `TC-CAT-001`, `BRL-CAT-02`, `BR-080`, `ADR-0027`) are cited by well over a thousand test titles, by commit messages and by Jira. For this project oxispec found no node (`E003` for every such file, `no node with id TC-CAT-001`), so there was no trace, no coverage and no link check.

What the project needs from oxispec, in the order of value:

1. an ID that stands for a row or a heading is a node, with a type, a title, some fields from other cells, and links from other cells (a test case verifies a user story);
2. the project's own ID shapes are accepted, and a range such as `TC-CAT-1200..1212` is read as the IDs it stands for;
3. its documents stay as they are: no split into thousands of files, no front matter added to each.

Things in oxispec that assume a node is a file: `Node.path`, the duplicate check (`E001`), `history` (git log of the file), `rollback` (puts a file back), `set-status` and `edit` (rewrite front matter), the static site (a page per file), the index (invalidated by file), suspect links (hash of the node's text) and `new` (writes a file). The ID parser cannot read `TC-CAT-1200..1212`: it splits at the first `.` and takes `.1212` for a child.

## Options

### 1. Split the documents into files (what exists)

`oxispec import specs --from markdown` makes a node per requirement, and a one-time script could do the same for rows. Nothing in the model changes.

For this project it means 1000+ files, the loss of the tables people read, and a document that is no longer where the team writes. The trial rejected it. It stays the answer for projects that start with oxispec.

### 2. Nodes sourced from a document, declared in the config

The config says which documents hold nodes and how to read them:

```yaml
sources:
  - files: "docs/02-features/*/test-cases.md"
    table:                       # a row of a table is a node
      id: 1                      # the column of the ID, matched by pattern
      id_pattern: "TC-[A-Z]+-[0-9]+"
      type: test-case
      title: 2
      links:
        verifies: 3              # the column holds IDs; a range is expanded
      fields: { priority: 5, level: 4 }
  - files: "docs/02-features/*/spec.md"
    heading:                     # a heading with an ID is a node
      id_pattern: "US-[A-Z]+-[0-9]+"
      type: user-story
```

A sourced node has an ID, a type, links and fields like any other node, and a place that is a line range of a file, not a file. The graph, the checks, `trace`, `coverage`, `show`, `search`, `list` and the site read it as they read any node. The document stays the only copy of the text.

What changes in the model: a `Node` gets `origin` (a file, or a part of a file with its first and last line). The parser of a sourced document is a second parser beside the front matter one. An ID that appears in a sourced position twice is `E001`, as now. A mention of the ID elsewhere is a reference, not a definition.

What a sourced node cannot do at first, with a message that says why: `set-status`, `edit`, `new` and `rollback` of one node (the file is the unit of git, so rolling back one row would roll back the table). `history` shows the history of the file filtered to the commits where the row changed, which is possible because the row can be parsed at each revision. Suspect links hash the row. The site shows the row on a page of its own.

Cost: the largest of the options, and a config that has to be right. The gain is that the project's documents work as they are, and the same mechanism reads any table or heading convention, so it is not written for one project.

### 3. Nodes sourced from a document, declared in the document

The same reading, but the document says it: an anchor on a heading (`### US-CAT-001 {#US-CAT-001}`, as a scenario has `{#S1}`) and a marker in the table (`<!-- oxispec: table type=test-case id=1 -->`).

It needs no config, and a document carries its own rules. But the project's files have to be edited, which is the cost it wants to avoid. It fits a project that starts with oxispec, and it can be added later as another way to fill the same `sources` structure.

### 4. References only: IDs without nodes

The config says what an ID looks like (`ids: { TC: "TC-[A-Z]+-[0-9]+" }`), and oxispec treats a match anywhere as a reference to something that exists. No node is made, so a link to it is not a dangling link, a test that names it is traced to it, and `trace TC-CAT-001` lists the tests, the documents and the code that mention it.

It is cheap, and `trace.test_ids` (iteration 92) is half of it. But there is no type, no title, no link from a column, no `check` of what a test case verifies, no coverage of a user story by a test case. It answers "where is this ID used" and not "is this requirement tested".

### 5. Generate the files from the documents

A command reads the sourced documents and writes a generated folder of nodes (in `.oxispec/` or git-ignored) that the graph reads as it reads any file. The model does not change: a generated node is a file whose path is the generated one, with `origin` as a field that points to the real document.

It reuses everything, including `history` and `site`, because the file exists. The costs: a build step that must run before `check` and that goes stale; line numbers and history point at generated files, so every finding names a place nobody edits; and two copies of the text on disk. It is a way to ship option 2's reading quickly, and a poor place to stay.

## Decision

Proposed: **option 2, in stages, with option 4's pattern IDs as the first stage**, and options 3 and 5 not taken.

1. **IDs of the project's own shape.** `ids:` declares patterns (`TC-[A-Z]+-[0-9]+`) that are valid IDs whatever their form, and a range `A..B` in a link column or in text is expanded to its IDs by a parser of its own that runs before `id::parse` (the ID parser is unchanged, so `.` still means a child). This stage alone gives option 4.
2. **Table sources.** `sources:` with `table:`, read-only, with `id`, `type`, `title`, `fields` and `links` by column. `check`, `trace`, `coverage`, `show`, `list`, `search`, `context` and the MCP tools read them. `Node` gets `origin`. Commands that write a node refuse one with a message that names the file and the row.
3. **Heading sources.** The same, for a heading with an ID.
4. **History, suspect links and the site** for sourced nodes: history by the row at each revision, suspect by the hash of the row, a page per node.
5. Writing a sourced node (`set-status`, `new`) is not planned. A project that wants it uses files.

Each stage is its own iteration and pull request, tested on that project's documents (one epic, then all of them) as an acceptance test.

External IDs such as Jira keys are a separate decision. They are a reference with a URL, not a node, so they follow stage 1 (a pattern, and a URL template to show a link), and a check that says whether a closed ticket has a test needs a source of the ticket's status, which oxispec does not have and does not fetch.

## Consequences

- ADR-0001 is amended, not replaced: Markdown files stay the source of truth. A node can be a part of one, and the file stays the unit of history and rollback.
- Every place that takes `Node.path` as a file needs a look: the list in the Context is the work. Most take the file and the node's line range without change. `edit`, `set-status`, `rollback` and `new` need a refusal with a clear message.
- The graph can have two kinds of node, which a user can tell apart (`origin` is in `show` and in the JSON). A check that works on the whole graph does not care.
- A config with a wrong column gives a wrong graph, so every source gets its own findings: a row without an ID, an ID that does not match its pattern, a duplicate, a link to a missing node.
- The format version stays 1: a project that has no `sources:` is read as before.
- It is more to maintain than the other options. The reason to accept it is that the largest class of projects that already write specs write them as headings and tables, and a tool that asks them to split every document will not be tried.
