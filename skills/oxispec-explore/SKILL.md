---
name: oxispec-explore
description: "Read the project's specs before writing or changing anything. Use to find requirements, decisions and their links, see how a node changed, or answer what the project has decided about a topic."
metadata:
  generated-by: oxispec
  content-hash: "170bb16fc52f8426"
---

# Explore the specs

Read what the project has already decided before you write or change anything. Every command below only reads.

## Steps

1. Find the nodes by their words: `oxispec search "session timeout"`. Narrow with `--type requirement` or `--status accepted`.
2. Read a node, its links and the nodes that link to it: `oxispec show REQ-AUTH-0001`. A node that links to many others is a good place to look for side effects.
3. If the node has links such as `decided-by`, `refines` or `governed-by`, show those too. The reason for a requirement is usually in its ADR.
4. See how a node got here: `oxispec history REQ-AUTH-0001`. To read an older version, use `oxispec show REQ-AUTH-0001 --at <git rev>`.
5. See what changed between two points: `oxispec diff <from> [<to>]`. Revisions can be a branch, a tag, a hash, or `date:YYYY-MM-DD`.
6. If the project traces code to specs, `oxispec trace REQ-AUTH-0001` lists the code and tests for a node.

## Rules

- Quote IDs exactly as printed, for example `REQ-AUTH-0001` or the scenario `REQ-AUTH-0001.S1`. Do not guess an ID. If `show` says it does not exist, search again.
- Add `--json` to any command when you need to process the output.
- State what you found with the IDs, so a person can open the same nodes.
