---
id: REQ-IMPORT-0002
type: requirement
status: accepted
date: 2026-11-06
---
### Requirement: A plan proposes what each file becomes
The system SHALL propose, for the files of another tool, a requirement for each spec and a change set for each open change or set of design notes, SHALL link a change set to the requirements it modifies, and SHALL allow notes to become a decision.

#### Scenario: A proposal {#S1}
- **WHEN** a folder has specs, an open change and design notes
- **THEN** the specs are proposed as requirements and the rest as change sets

#### Scenario: A change set {#S2}
- **WHEN** an open change has a proposal, a design, tasks and spec deltas
- **THEN** they are kept in the change set, which links the modified requirements

#### Scenario: A decision {#S3}
- **WHEN** notes are set to become a decision
- **THEN** the text is kept as the context of a decision

#### Scenario: Specs that are imported already {#S4}
- **WHEN** a plan is started after `import specs` has written the specs
- **THEN** those specs are in the plan as skipped, so applying it does not write them twice
