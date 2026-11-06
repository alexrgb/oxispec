---
name: oxispec-review-security
description: "Review a feature, an API or a design for security with the OWASP API Security Top 10 or ASVS as the lens, and record what is found as risks for the person to rate and accept. Use when asked for a security review, a threat check, or whether an API is safe."
metadata:
  generated-by: oxispec
  content-hash: "5c297cfb79f7d5a7"
---

# Review a design for security

Walk a feature, an API or a design through a short list of security questions, and record what you find as risks. You ask and propose. The person decides the likelihood, the impact and whether a risk is accepted.

## Steps

1. Read what exists: `oxispec risks` for the register, `oxispec api show "<call>"` for an operation, `oxispec search "<topic>"` for the requirements and decisions.
2. Pick the lens. For an HTTP API use the categories of the OWASP API Security Top 10 (access to another user's object, authentication, access to properties of an object, resource consumption, access to functions, abuse of business flows, server-side requests, misconfiguration, inventory of versions and hosts, trusting other APIs). For an application use the chapters of the OWASP Application Security Verification Standard. Check the current edition on owasp.org before you rely on its numbers; refer to a category by name and link to it.
3. For each operation or component, ask: who may call it, and what happens to someone who may not? Is the object or property in the request checked against the caller? What stops a caller from asking for too much, too often? What does it call out to, and what does it trust from there? Where are secrets and personal data kept and logged?
4. Where the answer is "nothing is specified", that is a finding. Where the answer is a behavior, check that a requirement and a scenario state it (for example, `the API responds with 403`) and that a test verifies it. `oxispec check` warns (W023) about an operation with no 401 or 403 scenario once other operations have one.
5. For each finding: `oxispec new risk "<what could go wrong>" --likelihood <low|medium|high> --impact <low|medium|high|critical> --from <REQ-ID>`. Ask the person for the likelihood and the impact. Link the mitigation (`mitigated-by`) when a requirement for it exists, or propose one with the `oxispec-propose` skill.
6. Show the person the list of new risks and the gaps. Run `oxispec check`.

## Rules

- Do not copy text from OWASP. Name the category, link to it, and say in your own words how it applies here.
- Do not mark a risk mitigated until the requirement that mitigates it exists and a test verifies it.
- Never accept a risk yourself. Never lower a likelihood or an impact to clear a warning or the release gate.
- You are reviewing a specification, not scanning code. Say so, and say what you could not see.
