# Roll specs back

Put nodes back to how they were at an earlier revision. Git keeps the history, so nothing is lost.

## Steps

1. Find the revision: `oxispec history REQ-AUTH-0001`, or `oxispec tag` for named points. A revision can be a hash, a branch, a tag or `date:YYYY-MM-DD`.
2. See the difference first: `oxispec diff <revision>`.
3. Try it without writing: `oxispec rollback <revision> REQ-AUTH-0001 --dry-run`. Use `--all` to restore every node that differs.
4. Run it without `--dry-run`. It refuses when the result would break links or lose uncommitted work.
5. Run `oxispec check` and review the files. Nothing is committed: commit the result yourself, with a message that names the revision.

## Rules

- Do not use `--force` unless a person says to.
- Rolling back a spec does not roll back the code. Check `oxispec trace` for the code that implements the node.
