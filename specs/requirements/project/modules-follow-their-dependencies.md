---
id: REQ-PROJECT-0002
type: requirement
status: accepted
date: 2026-11-08
---
### Requirement: Modules follow their dependencies
The system SHALL enable and disable modules with `oxispec modules`, refuse to disable a module another enabled module needs, refuse a preset that leaves out a module the others need, and refuse a command whose module is off.

#### Scenario: Add and remove {#S1}
- **WHEN** the tests module is added and the specs module is then removed
- **THEN** the removal is refused and names the module that needs it, and after the tests module is removed the specs module can go, and `new requirement` then says the module is not enabled

#### Scenario: A preset {#S2}
- **WHEN** `init` is given a set of modules with one a chosen module needs
- **THEN** the command exits with 2 and says which module needs it
