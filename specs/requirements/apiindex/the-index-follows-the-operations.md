---
id: REQ-APIINDEX-0002
type: requirement
status: accepted
date: 2026-11-06
---
### Requirement: The index follows the operations
The system SHALL embed only the chunks that are new or whose text changed, keep the others, drop the operations that are gone, and start again when the embedder changes.

#### Scenario: Only what changed {#S1}
- **WHEN** one chunk is new, one changed, one gone and one unchanged
- **THEN** only the new and the changed are embedded and the counts say so

#### Scenario: A new embedder {#S2}
- **WHEN** the embedder is another
- **THEN** every chunk is embedded again
