# Apply a change set to the code

Implement an approved change set, and leave the code tied to the specs.

## Steps

1. Read the change set and every node it changes: `oxispec show CHG-0001`, then `oxispec show` on each linked node. Check that the change set is `approved`. If it is not, stop and say so.
2. Write the code. Put a comment on the function that implements a requirement:
   ```text
   // spec: REQ-AUTH-0001
   ```
3. Write a test for each scenario, and name the scenario in a comment or in the test name:
   ```text
   // verifies: REQ-AUTH-0001.S1
   ```
   or `it('ends idle sessions [REQ-AUTH-0001.S1]', ...)`. The scenario needs an anchor (`{#S1}`) in the spec.
4. Run the tests with the project's own test command, and import the results when the project keeps them: `oxispec results import junit.xml`.
5. Run `oxispec trace REQ-AUTH-0001`, then `oxispec coverage`. Every scenario of the change should be verified.
6. Run `oxispec check`. Fix errors, including `E014` and `E015` (a comment names a node or scenario that does not exist).
7. Confirm the links you touched on purpose with the `oxispec-sync-code` skill.
8. When the work is merged, a person moves the change set to `merged`.

## Rules

- If the code cannot follow the spec, change the spec through a change set. Do not leave them different.
- Do not mark a scenario as verified with a test that does not exercise it.
