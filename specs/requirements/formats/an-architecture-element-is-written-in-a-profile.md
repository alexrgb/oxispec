---
id: REQ-FORMATS-0003
type: requirement
status: accepted
date: 2026-11-16
---
### Requirement: An architecture element is written in a profile
The system SHALL offer a built-in format profile for each kind of architecture element (`c4-person`, `c4-system`, `c4-container`, `c4-component`, `c4-environment`), write a new element in the profile that `--format` or the config names with its `architecture` block after the sections, refuse a profile made for another kind, and require the sections of the chosen profile with E028.

#### Scenario: A new element {#S1}
- **WHEN** a container is made with `--format c4-container`, and then a system is made with that same profile
- **THEN** the container has the sections of the profile and an `architecture` block, and the system is refused with a message that the profile is for containers

#### Scenario: A required section {#S2}
- **WHEN** the config chooses `c4-container` for containers and a container lacks the Responsibilities section
- **THEN** `check` reports E028 for it, and the profile is listed by `formats list`
