# Review a change to the specs

Review a branch or pull request that changes specs or code that is tied to specs.

## Steps

1. See what the branch introduces: `oxispec ci --base origin/main`. It reports the findings the change adds, and the nodes it touches and the nodes that link to them.
2. Read the graph difference: `oxispec diff origin/main`. Removed requirements and scenarios are breaking.
3. For each changed node, read it (`oxispec show ID`) and the nodes that link to it. Ask whether those nodes still hold.
4. Check the change set, if any: `oxispec change list`. Its status should match the work, and approval belongs to a person.
5. Run `oxispec suspect`. A changed function with a suspect link needs a decision from the author.
6. Run `oxispec coverage --status accepted` and look for new gaps.

## Report

- List problems by ID, with the file and what you would change.
- Say what you could not check.

## Rules

- Do not approve a change set, and do not merge.
- Treat a removed scenario as a breaking change even when no test fails.
