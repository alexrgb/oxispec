---
id: REQ-LINK-0001
type: requirement
status: accepted
date: 2026-11-07
---
### Requirement: A link is added to the node and nothing else changes
The system SHALL add a typed link from one node to another with `oxispec link`, check that the kind is known and that the target node or scenario exists, say so when the link is already there, and leave the rest of the file as it was.

#### Scenario: A new link {#S1}
- **WHEN** `link REQ-AUTH-0001 decided-by ADR-0001` is run, and then run again
- **THEN** the first run adds the link under `links:` in the front matter, the second says the node already has it, the target may be a scenario such as `REQ-AUTH-0001.S1`, and the text of the node is untouched

#### Scenario: A link that cannot be made {#S2}
- **WHEN** the target is missing, the scenario does not exist, the kind is unknown, or a node is linked to itself
- **THEN** the command exits with 2, says which of these it is, and writes nothing
