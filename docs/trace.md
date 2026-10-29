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

## Not built yet

Binding a link to a symbol with tree-sitter and its body hash, **suspect links** that go stale when either end changes, `oxispec confirm`, test result import (JUnit, `go test -json`) so coverage can say "verified and passing", coverage in `oxispec ci` and release gates, commit trailers (`Spec: CHG-0099`), and Go, Rust and Node package detection. Those are iteration 8b. A cache of scan results will come if scanning is measured to be too slow on a very large repository.
