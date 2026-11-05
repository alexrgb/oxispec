# Code and tests

Tie code and tests to the requirements with short comments, and OxiSpec tells you which requirements nothing implements and which scenarios no test checks.

## Mark the code and the tests

```rust
// src/session.rs
// spec: REQ-AUTH-0001
fn expire() { /* ... */ }
```

```rust
// tests/session.rs
// verifies: REQ-AUTH-0001.S1
#[test]
fn idle_admin() { /* ... */ }
```

`spec:` marks code that implements a requirement, `verifies:` marks a test. A test name can also carry the scenario in brackets, `[REQ-AUTH-0001.S1]`. The comment syntax of any language works. Details and the rules are in [trace](../trace.md).

## See the ties

```bash
oxispec trace REQ-AUTH-0001
```

```text
REQ-AUTH-0001  Admin session timeout  (draft)
implemented by:
  src/session.rs:1
scenarios:
  S1           Idle admin: verified by tests/session.rs:1
```

```bash
oxispec coverage
```

```text
requirements: 1 implemented of 1 (100%)
scenarios:    1 verified of 1 (100%)
2 annotations in 2 files
```

`coverage --status accepted` leaves out drafts, and `coverage --fail-under 80` exits with code 1 when either figure is below 80, which suits CI.

## Passing, not only present

Import test results to see which scenarios have a test that passes:

```bash
oxispec results import junit.xml     # or go test -json output
oxispec coverage --require-pass
```

## When code or specs move on

`oxispec suspect` lists links where the code or the spec changed after a person last confirmed they agree. Look at the pair, and if they still agree:

```bash
oxispec confirm REQ-AUTH-0001
```
