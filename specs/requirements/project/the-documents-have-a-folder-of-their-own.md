---
id: REQ-PROJECT-0004
type: requirement
status: accepted
date: 2026-11-15
---
### Requirement: The documents have a folder of their own
The system SHALL write the documents of a project (requirements, decisions, change sets, releases and the rest) in the folder that `docs_dir` names in the config, SHALL use `oxispec` for a project that `init` creates, and SHALL keep `specs` for a project whose config has no `docs_dir`.

#### Scenario: A new project {#S1}
- **WHEN** a project is initialized and a requirement is created
- **THEN** the config says `docs_dir: oxispec` and the node is in `oxispec/requirements`

#### Scenario: A folder of choice {#S2}
- **WHEN** a project is initialized with `--docs-dir docs/spec`
- **THEN** every command that writes a document writes it below `docs/spec`, including imports and releases

#### Scenario: A project from before {#S3}
- **WHEN** the config has no `docs_dir`
- **THEN** documents are written in `specs` and the architecture model in `architecture/model`, as before

#### Scenario: A folder outside the project {#S4}
- **WHEN** the folder is absolute, reaches out with `..`, or is hidden such as `.git`
- **THEN** `init` and every command that reads the config refuse it
