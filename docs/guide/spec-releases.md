# Releasing the specs

A release of the specs is a named state of them, tagged in git as `specs-vX.Y.Z`, with notes of what changed and a check that it is ready. It is separate from releasing the program. It needs the `releases` module (`oxispec modules add releases`, or the `full` preset).

## Prepare

```bash
oxispec release prepare
```

```text
no earlier release
suggested version: 0.1.0
wrote REL-0001 (specs/releases/0.1.0.md)
review the notes, commit the file, then run `oxispec release check 0.1.0` and `oxispec release publish 0.1.0`
```

The command compares the specs with the last release and writes a release node: what is breaking, what was added, what changed, and which change sets are included. It suggests a version by SemVer: removing a requirement is breaking, adding one is a minor change, anything else is a patch. Read the notes, edit them, and commit the file.

## Check

```bash
oxispec release check 0.1.0
```

```text
pass not-released             0.1.0 has not been released
pass version-increases        there is no earlier release
pass has-changes              the specs changed since the last release
pass no-errors                oxispec check reports no errors
pass change-sets-approved     0 change set(s), all approved or merged
pass manifest-current         the manifest lists the change sets since the last release
pass version-matches-changes  0.1.0 is at least the suggested 0.1.0
ready to publish
```

A gate marked `fail` blocks the release; some are advisory and only warn. More gates appear when the project asks for them: coverage and passing tests when tests are traced, `debt-within-limits` when the config sets `debt.limits`, and `compliance-met` when it has a `compliance:` section (see [compliance](../compliance.md) and [debt](../debt.md)).

## Publish

```bash
oxispec release publish 0.1.0
```

runs the gates again and creates the tag. It refuses to run if the release file is not committed yet, so commit `specs/releases/0.1.0.md` first. Tags are never moved. Push it as any tag: `git push origin specs-v0.1.0`.

## Use the releases

- `oxispec diff specs-v0.1.0` shows what changed since that release.
- `oxispec site --out public --releases` builds a site with a folder for each release and a version picker (for the nodes the project publishes, see [site](../site.md)).
- `oxispec rollback specs-v0.1.0 REQ-AUTH-0001 --dry-run` shows what putting a node back to that release would do. It checks that nothing breaks and commits nothing.

The rules for versions, and what counts as breaking, are in [versioning](../versioning.md). Releasing the program itself (binaries, crates.io, npm) is a different process, described in [releasing](../releasing.md).
