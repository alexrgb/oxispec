# Trace and coverage

Code and tests tie themselves to the specs with short comments. `oxispec trace` shows the ties, and `oxispec coverage` says which requirements nothing implements and which scenarios no test checks. Both need the `tests` module (`oxispec modules add tests`, included in the `standard` preset).

## Annotations

```rust
/// spec: REQ-AUTH-0012
fn expire_admin_session() {}
```

```go
// verifies: REQ-AUTH-0012.S1
func TestIdleAdmin(t *testing.T) {}
```

```ts
it('ends idle sessions [REQ-AUTH-0012.S1]', () => {})
```

- **`spec: ID`** marks code that implements a requirement. **`verifies: ID`** marks a test. Several IDs can follow, separated by commas: `// spec: REQ-A-0001, REQ-A-0002`. Text after the IDs is ignored. `Spec:` and `Verifies:` work too.
- **A scenario ID in brackets** (`[REQ-AUTH-0012.S1]`) anywhere on a line counts as `verifies`, which suits test names. It must name a scenario, so plain lists such as `[1, 2]` or `[REQ-A-0001]` are not read.
- A marker only counts **inside a comment**, in any language that uses `//`, `#`, `/* */`, `--`, `<!-- -->`, `;`, or docstring quotes. There is no parser per language, so a marker in a string that looks like a comment would be read too. IDs must start with capital letters (`REQ`, `ADR`, `CHG`), so prose like `spec: see-above` is left alone.
- A **scenario can only be named if it has an anchor**: `#### Scenario: Idle admin {#S1}`.
- **Links in nodes count as well.** A `test-case` node with `links: {verifies: REQ-AUTH-0012.S1}` covers that scenario, like a comment does.

## What is scanned

Every text file under the project that git does not ignore, except markdown (those are the specs), files over 1 MB, and files that are not UTF-8. Leave more out with globs in `.oxispec/config.yaml`:

```yaml
trace:
  exclude: ["vendor/**", "**/*.generated.*"]
```

The walk uses all cores and nothing is cached. On 50,000 source files (195 MB, 10 cores, macOS), a scan takes about 2 s.

## Commands

```bash
oxispec trace                       # every annotation found
oxispec trace REQ-AUTH-0012         # code and tests for one node, scenario by scenario
oxispec coverage                    # percentages, and what is missing
oxispec coverage --status accepted  # a draft is not expected to be built yet
oxispec coverage --fail-under 80    # exit 1 when either percentage is lower
oxispec coverage --json
```

- **Requirements implemented**: requirements with at least one `spec:` that names them or one of their scenarios.
- **Scenarios verified**: anchored scenarios that a `verifies` names. A `verifies` that names a requirement without a scenario covers all of its scenarios, which is coarser, so prefer the scenario ID.
- **Scenarios without an anchor** cannot be named, so they are counted apart.
- `coverage` exits with 1 on a reference to something that does not exist (below), or under `--fail-under`.

## New check codes

| Code | Meaning |
|---|---|
| E014 | A `spec:` or `verifies:` comment refers to a node that does not exist |
| E015 | A comment refers to a scenario that does not exist |

They are reported by `coverage` with the file and line, not by `oxispec check`, which only reads the specs.

## Suspect links

A `spec:` or `verifies:` comment is a link between a spec and a piece of code. With tree-sitter parsers for **Go, Rust, TypeScript and JavaScript**, OxiSpec knows which function, type or test the comment is about, and hashes it. When a person confirms a link, `.oxispec/trace.lock.json` records the hash of the code and of the spec text. Afterwards the link is **suspect** if either one changed, until someone looks and confirms it again. This is the main way code and specs stay in step.

```bash
oxispec suspect                  # suspect links (exit 1), and what is unconfirmed
oxispec suspect --strict         # unconfirmed links and orphans fail too
oxispec confirm REQ-AUTH-0012    # the links to this node are right, as they are now
oxispec confirm --all            # confirm everything (a good first step when adopting this)
oxispec confirm --all --dry-run
```

- **Which code.** A comment above a function, type or test binds to it. A comment inside a function binds to that function. Above a method in an `impl`, a class or a `describe`, it binds to the method. A scenario ID in a test title binds to the test (`it`, `test`). For a language without a parser the link has no code side, and only the spec is watched.
- **What counts as a change.** The hash covers the code without comments, blank lines or indentation, so reformatting and editing comments do not make a link suspect. Any other change does, including spaces inside a string. The spec side is the hash of the node's whole text after the frontmatter, so editing any scenario of a requirement makes every link to it suspect.
- **States.** `ok`; suspect because the `spec`, the `code` or `both` changed; `unconfirmed` (never confirmed, not a failure unless `--strict`); `orphaned` (confirmed once, but the annotation is gone, for example because its function was renamed; `confirm` drops it).
- **Renames.** A link is identified by its kind, target, file and symbol name, so renaming a function or moving it to another file shows as an orphan plus an unconfirmed link. That is on purpose: a person confirms the new place. Two tests with the same title in one file get `#2`, `#3`.
- **Confirming is a statement by a person** that the code still satisfies the spec. It is recorded in a file under git, so it is reviewed like code. An agent should list suspect links and leave confirming to a person.
- **`oxispec ci`** reports every suspect link as error E016 (with the file and line), so a pull request that changes code or a spec without confirming fails. A project that never confirmed anything has no suspect links, so adopting this does not fail anything by itself.

| Code | Meaning |
|---|---|
| E016 | A link between a spec and code is suspect: either one changed since it was confirmed |

## Not built yet

Test result import (JUnit, `go test -json`) so coverage can say "verified and passing", suspect links as a release gate and in coverage output, pinning a release to the code commit, commit trailers (`Spec: CHG-0099`), package detection for Go, Rust and Node, and symbols for Python and other languages. The spec side is coarser than the code side: it is the whole node, not the one scenario. A cache of scan results will come if scanning is measured to be too slow on a very large repository.
