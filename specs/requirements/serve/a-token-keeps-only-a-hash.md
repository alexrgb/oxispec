---
id: REQ-SERVE-0001
type: requirement
status: accepted
date: 2026-11-07
---
### Requirement: A token keeps only a hash
The system SHALL create tokens for `oxispec serve` that begin with `oxs_`, keep only their hash in a file git ignores, refuse a second token with the same name, and list and revoke them by name.

#### Scenario: Create, list, revoke {#S1}
- **WHEN** a token is created, listed and revoked
- **THEN** the secret is shown once, the file holds only a hash, the folder is ignored by git, a name cannot be used twice, and a revoked token is gone from the list
