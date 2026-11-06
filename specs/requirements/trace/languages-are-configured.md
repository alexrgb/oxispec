---
id: REQ-TRACE-0004
type: requirement
status: accepted
date: 2026-11-06
---
### Requirement: The languages of the scan are configured
The system SHALL let a project limit the trace scan to the languages it names, add a language without a grammar by naming its file extensions and comment leaders, read such a language by those leaders, and report a setting that names an unknown language, a language with a grammar, or a new language with no extensions or leaders.

#### Scenario: Limiting and adding {#S1}
- **WHEN** a project names the languages `rust` and a new `zig` with the extension `zig` and the leader `//`
- **THEN** only Rust and Zig files are scanned, and without any such setting every file is

#### Scenario: A bad setting {#S2}
- **WHEN** the setting names `cobol`, gives leaders for `rust`, or adds `zig` with no extensions
- **THEN** it is refused with the reason, while the leaders of a built-in language without a grammar, such as `python`, can be changed
