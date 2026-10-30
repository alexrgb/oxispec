---
name: oxispec-release
description: "Prepare, check and publish a release of the specs: manifest, gates and tag. Use when asked to cut a spec release."
metadata:
  generated-by: oxispec
  content-hash: "d3641b98f091cce4"
---

# Release the specs

A release is a manifest node plus a git tag `specs-v<version>`. The gates decide whether it is ready.

## Steps

1. See what would go in: `oxispec release prepare --dry-run`. The suggested version follows the changes (breaking, added, changed).
2. Write the manifest: `oxispec release prepare`, or `oxispec release prepare 0.2.0` to choose the version. Commit it.
3. Check the gates: `oxispec release check 0.2.0`. Exit code 1 means it is not ready, and each failed gate says why.
4. Fix what failed. Common ones: a change set that is not approved, an error from `oxispec check`, a suspect link, a failing test.
5. Publish: `oxispec release publish 0.2.0`. It runs the gates again on the last commit and tags it.

## Rules

- Do not publish without a person's go-ahead. Publishing creates a tag.
- Do not change a gate's result by editing the manifest by hand.
- `oxispec release list` shows which releases exist and whether they are published.
