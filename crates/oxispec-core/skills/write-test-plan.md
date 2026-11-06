# Write a test plan

A test plan says what is tested, how and where, and when testing is done. It is a node (`test-plan`) that `covers` requirements and risks, and it is read against the tests that exist. You write it with the person; you do not approve it.

## Steps

1. Look first: `oxispec test-plan` lists the plans and their gaps. Extend one instead of starting another for the same scope.
2. Ask what the plan is for: a release, a feature, a risk. List the requirements in scope with `oxispec search` and the live risks with `oxispec risks`.
3. Create it: `oxispec new test-plan "Checkout release 1.2"`. Fill in Scope (including what is not tested, and why), Approach, Levels (unit, integration, end to end, security, performance: which apply and where each runs), Environments and Exit criteria.
4. Link what it covers: `oxispec link TP-0001 covers REQ-PAY-0001` and `oxispec link TP-0001 covers RISK-0003`.
5. Read it against the tests: `oxispec test-plan TP-0001`. For each requirement it names the scenarios no test verifies, and the ones that fail. For each risk it says whether the requirements that mitigate it are tested.
6. For a gap, write the test (`// verifies: REQ-PAY-0001.S2`) or say in the plan why there is none. Do not remove a requirement or a risk from `covers` to make a gap disappear.
7. Keep the status `draft`. A person sets `approved`. Then `oxispec check` warns (W021, W022) about accepted requirements and live high risks that no approved plan covers.

## Rules

- A plan is not a test. Linking a plan to a requirement does not verify it; only a test with `verifies:` does.
- Exit criteria are statements a person can check (for example, every scenario of the covered requirements passes), not intentions.
- Do not invent environments or tools the project does not have. Ask.
