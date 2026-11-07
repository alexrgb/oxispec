---
id: REQ-RESULTS-0001
type: requirement
status: accepted
date: 2026-11-07
---
### Requirement: Test results say which scenarios pass
The system SHALL import test results (JUnit XML or `go test -json`), store them, and let coverage count only the scenarios whose tests pass when asked with `--require-pass`, and SHALL say so when `--require-pass` is used before any results are imported.

#### Scenario: Pass and fail {#S1}
- **WHEN** a run of results is imported and coverage is asked with `--require-pass` and a threshold
- **THEN** coverage says which scenarios pass, the percentage that counts is the passing one, a failing test fails the run with `--require-pass` only, and before any import the command exits with 2
