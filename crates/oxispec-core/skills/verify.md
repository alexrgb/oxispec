# Verify that the specs and the project agree

Run the checks in the order below. Each one answers a different question.

## Steps

1. `oxispec check`: are the specs consistent (links, IDs, statuses, and claims that contradict each other)? Exit code 1 means errors. Add `--strict` to treat warnings as errors.
2. `oxispec coverage`: which requirements has no code named, and which scenarios has no test named? Add `--status accepted` to ignore drafts.
3. `oxispec coverage --require-pass`: the same, counting only scenarios whose tests passed in the imported results. Import them first with `oxispec results import FILE...` (JUnit XML or `go test -json`).
4. `oxispec suspect`: which links between specs and code changed since they were confirmed?
5. On a branch, `oxispec ci --base origin/main` reports only the findings this branch introduces.

## Reading the result

- Report each finding with its code (for example `E002`) and the ID it names.
- Fix what is yours to fix. For a missing test, write the test. For a suspect link, read both sides before you confirm.
- Say clearly what you did not run, for example "no test results were imported".

## Rules

- Never hide a failure by changing a status, deleting a link, or editing the lock file.
- A green `check` does not mean the code is right. It means the specs agree with themselves.
