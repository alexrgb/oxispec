# History

Your specs live in git, so git already keeps every version, who changed it, and when. OxiSpec reads that history and explains it in terms of specs: which node changed, which link was added, which requirement was removed. Nothing is stored apart from git, so there is no second copy to fall out of step.

These commands need a git repository.

## Naming a revision

Wherever a command takes a revision, you can give:

| You type | Meaning |
|---|---|
| `HEAD`, `HEAD~3`, `main`, `a1b2c3d` | Anything git understands |
| `v1.0` | A tag. If there is no such revision, the tag `specs-v1.0` is tried, which is how spec release tags are named (see [versioning.md](versioning.md)) |
| `date:2026-01-31` | The last commit on or before that day. Git cannot read dates after 2099. |

## Reading the past

```bash
oxispec show REQ-AUTH-0001 --at v1.0     # the node as it was
oxispec check --at date:2026-01-31       # were the specs consistent then?
```

Both print the revision they used. `--at` cannot be combined with `check --changed`. The JSON output has an `at` object with the commit, its short form and its subject, or `null` for the working tree.

## History of a node

```bash
oxispec history REQ-AUTH-0001
```

```text
REQ-AUTH-0001  specs/requirements/auth/session-timeout.md
  uncommitted
      text changed
  1953dc2  2026-10-28  Ann  rename file
      moved: specs/requirements/auth/admin-session-timeout.md -> specs/requirements/auth/session-timeout.md
  ef17c3e  2026-10-28  Ann  accept the timeout requirement
      status: draft -> accepted; link added: decided-by ADR-0001
  efd148b  2026-10-28  Ann  add auth requirement and adr
      created
```

Each entry is a commit that changed the node's file, newest first, followed across renames and moves. Under it is what changed in terms of the node: type, status, owner, path, links added and removed, requirements and scenarios added and removed, and whether the text changed. An edit that is not committed yet is listed first.

- `--limit N` shows N commits (default 20) and says when there is more.
- `--at REVISION` reads the history up to that revision, and finds nodes that no longer exist in the working tree. Without it, a deleted node is reported as unknown, with a hint to use `--at`.
- `--json` gives `entries`, each with the commit (or `null` for uncommitted), an `event` of `created`, `changed`, `deleted` or `uncommitted`, and the list of `changes`.

An edit that only changes a comment in the frontmatter shows as "no change to the node itself".

## Comparing two revisions

```bash
oxispec diff v1.0              # from v1.0 to the working tree
oxispec diff v1.0 v2.0         # between two revisions
```

```text
from efd148b (add auth requirement and adr) to the working tree
+ REQ-AUTH-0002  specs/requirements/auth/password-length.md
~ REQ-AUTH-0001  specs/requirements/auth/session-timeout.md
    status: draft -> accepted
    link added: decided-by ADR-0001
findings: 0 new, 0 fixed
```

`+` is a node added, `-` removed, `~` changed. Then the checks are run on both sides and the difference is reported: errors and warnings that are new, and ones that went away. A finding is matched by its node and message, not its file, so moving a file does not make its findings new. This is how you answer "did this release make the specs less consistent?".

`--json` gives `added`, `removed`, `changed` (with the same `change` objects as `history`), `new_findings` and `fixed_findings`.

## Naming a revision with a tag

```bash
oxispec tag v2.3                       # tag the latest commit as specs-v2.3
oxispec tag v1.0 --at HEAD~5 -m "first release"
oxispec tag                            # list the spec tags
```

A spec tag is an annotated git tag named `specs-<name>`, so it never collides with the tags of your code (see [versioning.md](versioning.md)). You use it by its short name, for example `oxispec show REQ-AUTH-0001 --at v2.3`. Tags are never moved or overwritten: a name that exists is an error. A tag points at a commit, so changes that are not committed are not in it, and the command says how many spec files are left out. `--json` works for creating and for listing. Annotated tags need git to know your name and email.

## Rolling back

```bash
oxispec rollback v2.3 REQ-AUTH-0001 ADR-0004   # put these nodes back as they were at v2.3
oxispec rollback v2.3 --all                    # restore everything that differs
oxispec rollback v2.3 REQ-AUTH-0001 --dry-run  # show what would happen, change nothing
```

```text
rolling back to 73f1fea (c1 create)
  restore REQ-AUTH-0001 (status: accepted -> draft; link removed: decided-by ADR-0001)
wrote 1 file(s), deleted 0. Review it with `oxispec diff HEAD`, then commit. ...
```

For each node, the rollback does one of these:

| The node | What happens |
|---|---|
| exists now and then, and differs | its file is put back as it was, at the path it had then |
| was deleted since | its file comes back |
| did not exist then | its file is deleted |
| is already the same | nothing |

**Nothing is committed.** The files change in your working tree, you review them with `oxispec diff HEAD`, and you commit. That commit is the new revision. History is never rewritten, and the rollback itself is visible in `oxispec history`.

**It checks before it writes.** It builds the specs as they would be afterwards and runs the checks. If that would introduce new errors, for example a deleted node that other nodes link to, it prints them and stops without touching anything. It also stops if any file it would change or delete has changes that are not committed, since that work cannot be recovered from git, and if a restored file would overwrite a file that is not part of the rollback. `--force` goes ahead anyway, and `--dry-run` reports the same problems without writing.

`--json` prints the actions and the problems found. The nodes to roll back must be named, or `--all` given, never both.

## How fast

Reading a revision means reading every spec file from git's object store, in one `git cat-file` process. On 20,000 requirement files (release build, macOS, Apple silicon, after the first run):

| Command | Time |
|---|---|
| `show --at` or `check --at` | about 1 to 1.5 s |
| `diff` between two revisions | about 2 to 3 s |
| `history` of one node | about 0.3 s |

Runs right after a very large commit can be much slower, because git may be packing its objects in the background. I saw a first run of 20 s. These are one machine's numbers, not a promise. If history queries over many commits become too slow, an index over commits can be added without changing the commands.
