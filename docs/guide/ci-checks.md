# Checks in CI

Run `oxispec check` on every pull request so a broken link or a requirement without a scenario is caught before it is merged. For a pull request the more useful command is `oxispec ci`: it reports only what the branch changes.

## Fail the build on errors

```bash
oxispec check
```

exits with code 1 when there are errors. That is enough for a first job.

## Only what the branch introduces

```bash
oxispec ci --base origin/main
```

```text
comparing with ea64e48 (init)
1 nodes changed, 1 touched: 0 new errors, 0 new warnings, 0 fixed, 0 existing not shown
```

It compares the specs on the branch with the point where the branch left `--base`, and lists the findings that are new, the ones the branch fixed, and the nodes it touched (changed ones, and the ones that link to them). Problems that were already in the project are not shown, so an old project can adopt the check without fixing everything first. `--strict` makes new warnings fail too.

## Reports for other tools

| Option | Writes | Use |
|---|---|---|
| `--json` | JSON on the output | scripts |
| `--markdown FILE` | a pull request comment | a sticky comment on the PR |
| `--sarif FILE` | SARIF 2.1.0 | GitHub code scanning |
| `--junit FILE` | JUnit XML | test dashboards |

The files are written even when the run fails, so give the upload steps `if: always()`.

## A GitHub Actions job

The full example, with the comment and code scanning, is in [CI](../ci.md#github-actions). The short form:

```yaml
name: specs
on: pull_request
jobs:
  oxispec:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v7
        with:
          fetch-depth: 0          # the comparison needs history
      - run: cargo install --git https://github.com/alexrgb/oxispec oxispec --locked
      - run: oxispec ci --base "origin/${{ github.base_ref }}"
```

`fetch-depth: 0` matters: with a shallow clone there is no branch point to compare with.

## Coverage as a gate

If code and tests carry `spec:` and `verifies:` comments (see [Code and tests](trace-and-coverage.md)), add:

```bash
oxispec coverage --status accepted --fail-under 80
```

It exits with code 1 when fewer than 80 percent of accepted requirements are implemented or fewer than 80 percent of their scenarios are verified.
