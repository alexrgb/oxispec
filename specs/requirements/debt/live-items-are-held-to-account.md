---
id: REQ-DEBT-0002
type: requirement
status: accepted
date: 2026-11-06
---
### Requirement: A live debt item is held to account
The system SHALL warn about a debt item that is still to be paid and has no owner or no source, is past its due day, or has not been reviewed within the configured days, and SHALL NOT warn about a resolved or accepted item.

#### Scenario: No owner or source {#S1}
- **WHEN** a live item has no owner and no `arises-from` link
- **THEN** the warnings W013 and W016 are reported

#### Scenario: Late or stale {#S2}
- **WHEN** a live item is past its due day, or was last reviewed longer ago than the configured days
- **THEN** W014 or W015 is reported, an item due today is not late, and a resolved or accepted item raises neither
