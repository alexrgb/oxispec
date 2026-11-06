---
name: oxispec-manage-risks
description: "Record a risk to the application, keep the register honest, and decide with the person what to do about each: owner, likelihood, impact, what mitigates it, who accepts it. Use when a threat or weakness is found, a design is reviewed for security, or you are asked what risks there are."
metadata:
  generated-by: oxispec
  content-hash: "a40b315d1c2767a3"
---

# Manage risks

Keep a register of the risks to the application, and make it say what is being done about each. You propose; the person decides the likelihood, the impact and whether a risk is accepted.

## Steps

1. Look first: `oxispec risks` lists what is live, the most serious first. `oxispec search "<topic>" --type risk` finds one on the same subject. Update it instead of writing another.
2. To add one: `oxispec new risk "Session tokens never expire" --likelihood medium --impact high --from REQ-AUTH-0001`. Fill in what the risk is, what would trigger it, and what it would cost. Ask the person for the likelihood and the impact. Do not guess them to make a risk look small.
3. Name an owner in the file. A risk without one raises W017.
4. Link what deals with it: `oxispec link RISK-0001 mitigated-by REQ-AUTH-0007`. The mitigation is a requirement, a decision or a test, not a promise. Then `oxispec risks status RISK-0001 mitigating`.
5. When it is dealt with, `oxispec risks status RISK-0001 mitigated`. Keep it as `mitigated` until the evidence (the requirement and its test) is in place.
6. A risk the team lives with is `accepted`. Never accept one yourself. Ask the person, and record who with `oxispec risks status RISK-0001 accepted --by "Name"`. E039 is raised if no one is named.
7. Run `oxispec check`.

## Rules

- A live risk at the critical level holds a release back. Say so when you add one.
- Set `review` to a date by which the risk must be looked at again. W018 fires after it.
- Never lower a likelihood or an impact to clear a warning or a release gate. Write what changed (a mitigation that exists) and let the person decide.
- Do not copy text from OWASP or other standards. Link to the control and say in your own words how it applies.
