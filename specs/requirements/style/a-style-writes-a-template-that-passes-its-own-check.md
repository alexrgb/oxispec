---
id: REQ-STYLE-0003
type: requirement
status: accepted
date: 2026-11-08
---
### Requirement: A style writes a template that passes its own check
The system SHALL write a new requirement in the style the project or `--format` chooses (shall, EARS, user story or Gherkin) so that it passes the check of that style, let `--format` override the config, refuse a style for another type, and report E031 for a custom profile whose style does not exist.

#### Scenario: Each style {#S1}
- **WHEN** a requirement is made in each style
- **THEN** it passes `check` and has the form of its style: SHALL, an EARS pattern, `As a <role>`, or `GIVEN`

#### Scenario: The flag {#S2}
- **WHEN** `new requirement --format user-story` runs in a project that chose another style, and with a bad name
- **THEN** the flag wins, and a bad name exits with 2

#### Scenario: A custom profile {#S3}
- **WHEN** a custom profile has a style, and then a style that does not exist
- **THEN** the first is held to it, and the second is E031
