---
id: REQ-PROJECT-0006
type: requirement
status: accepted
date: 2026-11-17
---
### Requirement: The documents of a project are fixed once
The system SHALL, with `oxispec onboard`, record the kinds of documents a project keeps in `documents` of the config and the format of each in `formats`, enable the modules they need, start the project when there is none, ask about each kind when no list is given, and then refuse `new` for a kind that is not in the list.

#### Scenario: A list given up front {#S1}
- **WHEN** `onboard --docs requirement:ears,adr:madr,risk` is run in an empty folder, and then a debt item, a Nygard-less ADR and a risk are made
- **THEN** the project is started with the modules those kinds need, the config has the list and the formats, the ADR is written in MADR, and a kind outside the list is refused with the list in the message

#### Scenario: Questions and a bad answer {#S2}
- **WHEN** `onboard` is run with answers on its input, and then with a kind or a format that does not exist
- **THEN** the answers decide the list and the formats, an empty answer keeps what the project has, and an unknown kind or format exits with 2 and changes nothing
