# Check codes

Written by `oxispec docs reference` from the program itself. Do not edit it by hand.

A code is never reused. `E` is an error (`oxispec check` exits with 1), `W` a warning and `I` a note. What each check looks at is described in [the file format](../format.md#check-codes).

| Code | Severity | Meaning |
|---|---|---|
| E001 | error | Two nodes have the same ID |
| E002 | error | A link points to a node that does not exist |
| E003 | error | Frontmatter has `id` or `type`, but not both |
| E004 | error | `supersedes` links form a cycle |
| E005 | error | An ID is not valid |
| E006 | error | A requirement has no scenario |
| E007 | error | A requirement has no SHALL or MUST |
| E008 | error | A requirement node has no `### Requirement:` block |
| E009 | error | A link points to a scenario that does not exist |
| E010 | error | A scenario anchor is used twice in one file |
| E011 | error | A scenario anchor is not letters and digits |
| E012 | error | A change set has no status, or one that is not in the lifecycle |
| E013 | error | A change set is approved or merged but has no `approved_by` |
| E014 | error | A code annotation refers to a node that does not exist |
| E015 | error | A code annotation refers to a scenario that does not exist |
| E016 | error | A link between a spec and code is suspect: either one changed since it was confirmed |
| E017 | error | A claim cannot be read |
| E018 | error | A vocabulary block cannot be read |
| E019 | error | Claims conflict: in some context, no value satisfies them all |
| E020 | error | A state machine cannot be read |
| E021 | error | A state machine names a state it does not declare |
| E022 | error | A state machine has two transitions on one event that can both apply |
| E023 | error | A state machine breaks a property, with the path that does it |
| E024 | error | A state machine links to a node that does not exist |
| E025 | error | An architecture block cannot be read |
| E026 | error | An architecture element, environment or rule refers to something that is not right |
| E027 | error | An architecture rule is broken, with the path that breaks it |
| E028 | error | A section that the node's format profile requires is missing |
| E029 | error | A front matter field that the node's format profile requires is missing |
| E030 | error | A status is missing or not one that the node's format profile allows |
| E031 | error | The config names a format profile that does not exist or is not valid |
| E032 | error | A debt item has a status, severity or date that is not valid |
| E033 | error | A requirement is not worded the way its style asks (EARS or user story) |
| E034 | error | A scenario does not have the steps its style asks (Gherkin) |
| E035 | error | A node lacks a link the project's compliance rules ask for |
| E036 | error | A node lacks the sign-off the project's compliance rules ask for |
| E037 | error | The compliance settings in the config are not valid |
| W001 | warning | Unknown node type |
| W002 | warning | Frontmatter is not valid YAML, so the file was skipped |
| W003 | warning | A file could not be read |
| W004 | warning | Unknown link kind |
| W005 | warning | A SHOULD claim cannot hold in a context where other claims apply |
| W006 | warning | A claim never applies, because its scope can never hold |
| W007 | warning | A state cannot be reached from the initial state |
| W008 | warning | A state has no way out and is not final |
| W009 | warning | A container has no owner or no technology |
| W010 | warning | A container runs in no deployment environment |
| W011 | warning | A decision is marked superseded, but no decision in force supersedes it |
| E038 | error | A risk has a status, likelihood, impact or date that is not valid |
| E039 | error | A risk is accepted but no person is named in `approved_by` |
| W012 | warning | A decision is superseded by another, but its status still says it is in force |
| W013 | warning | A debt item that is still to be paid has no owner |
| W014 | warning | A debt item is past its due date |
| W015 | warning | A debt item has not been reviewed for longer than the project allows |
| W016 | warning | A debt item does not say what it arises from |
| W017 | warning | A risk that is still live has no owner |
| W018 | warning | A live risk is past its review date |
| W019 | warning | A risk is being mitigated but no `mitigated-by` link says by what |
| I001 | info | A link to another project is not checked |
| I002 | info | A claim says nothing that another claim does not already say |
