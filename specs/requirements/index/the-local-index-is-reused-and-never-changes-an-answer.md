---
id: REQ-INDEX-0001
type: requirement
status: accepted
date: 2026-11-08
---
### Requirement: The local index is reused and never changes an answer
The system SHALL keep a local index under `.oxispec/cache` that parses only the files that changed, SHALL give the same answers with the index, without it and on the first run, SHALL leave nothing behind when `--no-cache` is given, and SHALL replace a damaged index without an error.

#### Scenario: Parse once {#S1}
- **WHEN** `index` is run, then run again, then run after one file changes and another is deleted
- **THEN** the first run parses every file, the second reads and parses none and writes nothing, and the third parses only the change

#### Scenario: The same answers {#S2}
- **WHEN** `check` is run cold, warm and with `--no-cache`
- **THEN** the three outputs are the same

#### Scenario: No cache {#S3}
- **WHEN** a command runs with `--no-cache`
- **THEN** no cache folder is made, and `index --no-cache` exits with 2

#### Scenario: A damaged index {#S4}
- **WHEN** the cache file is damaged
- **THEN** the next command succeeds and the file is valid JSON again
