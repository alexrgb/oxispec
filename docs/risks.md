# Risks

A register of the risks to the application, kept as nodes of type `risk` in `specs/risks/`. It needs the `risks` module (`oxispec modules add risks`, included in the `standard` preset). A risk is a node and not a line in a spreadsheet, so it has an owner, can be linked to what deals with it, and can hold a release back.

OxiSpec does not find risks and is not a security scanner. It keeps the register honest: who owns each risk, when it was last looked at, what mitigates it, and who accepted it.

## A risk

```markdown
---
id: RISK-0003
type: risk
status: mitigating
likelihood: medium
impact: high
owner: platform-team
date: 2026-11-06
review: 2027-02-01
links:
  arises-from: REQ-AUTH-0001
  mitigated-by: [REQ-AUTH-0007]
---
# RISK-0003: Session tokens never expire
```

| Field | Meaning |
|---|---|
| `status` | `open`, `mitigating`, `mitigated`, `accepted` or `closed`. The first two are *live*. |
| `likelihood` | `low`, `medium` or `high`. |
| `impact` | `low`, `medium`, `high` or `critical`. |
| level | Not written. Worked out from the two, see below. |
| `owner` | A person or a team. |
| `date` | When the risk was opened or last reviewed. `risks status` sets it to today. |
| `review` | The day by which it must be reviewed again. Optional. |
| `approved_by`, `approved_on` | Who accepted the risk, and when. Written by `risks status ... accepted`. |
| `mitigated-by` (link) | The requirement, decision or test that deals with it. |
| `arises-from` (link) | What the risk comes from. |

`oxispec new risk "Session tokens never expire" --likelihood medium --impact high --from REQ-AUTH-0001` writes the file, with `owner` and `review` empty for you to fill in.

## The level

The level is the product of the places of the likelihood (1 to 3) and the impact (1 to 4) on their scales: up to 2 is **low**, up to 4 **medium**, up to 8 **high**, and above that **critical**. So high likelihood with high impact is critical, medium with critical is high, and low with critical is medium. Nobody writes the level, so it cannot be set low to clear a warning. The scales and the cut-offs are fixed for now.

## The register

`oxispec risks` lists the live risks, the most serious first and, within a level, the oldest first. A risk past its review day is flagged. `--status`, `--level` and `--all` (adds mitigated, accepted and closed) narrow or widen it, and `--json` gives the same for scripts.

`oxispec risks status RISK-0003 mitigated` moves a risk and records today as the day it was reviewed. `oxispec risks status RISK-0003 accepted --by "Dana"` accepts it and records who (without `--by`, git's `user.name`).

## Checks

| Code | Meaning |
|---|---|
| E038 | The status, likelihood or impact is missing or not valid, or `date` or `review` is not a date as `YYYY-MM-DD`. |
| E039 | A risk is `accepted` and `approved_by` names no one. |
| W017 | A live risk has no owner. |
| W018 | A live risk is past its `review` date. |
| W019 | A risk is `mitigating` or `mitigated` and has no `mitigated-by` link. |

A risk that is `mitigated`, `accepted` or `closed` raises none of W017 and W018. The checks use today's date. They are part of `check`, so `oxispec ci` reports them like any other finding.

## Releases

When the project has any risk, `oxispec release check` has the gate `no-critical-risks`. It fails while a live risk is at the critical level, and names it. A risk that is mitigated, accepted by a person or closed does not count. The threshold is fixed, not set in the config.

## With an AI agent

The skill `oxispec-manage-risks` records a risk, asks for the likelihood and impact instead of guessing them, links the mitigation, and leaves acceptance to a person.

## What is not built

There is no MCP tool for the register yet, no link kind between a risk and a component of the architecture model, and no catalog of risks from a standard such as the OWASP ASVS. Those come after this.
