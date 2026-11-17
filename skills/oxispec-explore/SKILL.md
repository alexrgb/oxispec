---
name: oxispec-explore
description: "Read the project's specs before writing or changing anything. Use to find requirements, decisions and their links, see how a node changed, or answer what the project has decided about a topic."
metadata:
  generated-by: oxispec
  content-hash: "0c13ac4ea4d460ce"
---

# Explore the specs

Read what the project has already decided before you write or change anything. Every command below only reads.

## Steps

1. Find the nodes by their words: `oxispec search "session timeout"`. Narrow with `--type requirement` or `--status accepted`.
2. To read everything the specs say about one functionality, use `oxispec read authentication` (a folder of requirements or the domain of their IDs; `oxispec read` alone lists them). It prints every requirement with its scenarios and the open change sets. To read one node, its links and the nodes that link to it: `oxispec show REQ-AUTH-0001`. A node that links to many others is a good place to look for side effects.
3. If the node has links such as `decided-by`, `refines` or `governed-by`, show those too. The reason for a requirement is usually in its ADR.
4. See how a node got here: `oxispec history REQ-AUTH-0001`. To read an older version, use `oxispec show REQ-AUTH-0001 --at <git rev>`.
5. See what changed between two points: `oxispec diff <from> [<to>]`. Revisions can be a branch, a tag, a hash, or `date:YYYY-MM-DD`.
6. If the project traces code to specs, `oxispec trace REQ-AUTH-0001` lists the code and tests for a node.

## Which document and which format

Before you create a document, run `oxispec onboard --show`. It lists the kinds of documents this project keeps, the format of each and the command that writes it. Use that command as it is: `oxispec new` takes the format from the config, so do not add `--format` and do not ask the person which document or format they want when the list answers it. A kind that is not on the list is refused by `new`; say so and suggest `oxispec onboard --docs <the kinds>`. When nothing is fixed yet, suggest `oxispec onboard` to the person instead of choosing for them.

In a monorepo, each product folder has a project of its own: run the commands inside the folder of the product you work on (`oxispec products` lists them). From the top, `list`, `read` and `search` see only the top project; `check` and `site` go through every product.

## Rules

- Quote IDs exactly as printed, for example `REQ-AUTH-0001` or the scenario `REQ-AUTH-0001.S1`. Do not guess an ID. If `show` says it does not exist, search again.
- Add `--json` to any command when you need to process the output.
- State what you found with the IDs, so a person can open the same nodes.
