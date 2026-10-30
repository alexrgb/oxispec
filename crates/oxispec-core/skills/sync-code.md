# Sync code and specs

A link between a spec and the code that implements it becomes suspect when either side changes after the link was confirmed. Decide which side is right, then confirm.

## Steps

1. List the suspect links: `oxispec suspect`. Each line names the node, the code location and which side changed (spec, code or both).
2. For each one, read the spec (`oxispec show ID`) and the code at the location it names.
3. If the code and the spec still agree, confirm: `oxispec confirm REQ-AUTH-0001`. Use `--dry-run` first to see what it would record.
4. If they disagree, fix the code, or propose a change to the spec (the `oxispec-propose` skill). Confirm after the fix.
5. A link that is not confirmed yet shows as such. A link whose annotation is gone (for example the function was renamed) is reported as orphaned. If the code moved, the new place shows as unconfirmed: confirm it, and `confirm` drops the orphan.
6. Commit `.oxispec/trace.lock.json` with the change. It is the record of what was confirmed.

## Rules

- Do not run `oxispec confirm --all` to make the list empty. Confirm what you checked.
- Link binding covers Go, Rust, TypeScript and JavaScript. For other languages, links are not tracked as suspect.
