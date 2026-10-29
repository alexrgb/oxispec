# Using OxiSpec in CI

`oxispec ci` checks a change, not the whole project. It compares the specs in the working tree with the specs where the branch left the base, and reports **only the findings the change introduced**. A project with old problems can still gate new work, and `oxispec check` stays the way to see everything.

```bash
oxispec ci --base origin/main
oxispec ci --base origin/main --strict                  # new warnings fail too
oxispec ci --base origin/main --json
oxispec ci --base origin/main \
  --sarif oxispec.sarif --junit junit.xml --markdown oxispec.md
```

- **Base.** `--base` defaults to `origin/main`. The comparison point is the merge base of that ref and `HEAD`, so a pull request is not blamed for work that landed on `main` after it branched. The history has to be there: with `actions/checkout` set `fetch-depth: 0`. A missing ref or a shallow clone stops with an explanation (exit code 2).
- **What is compared.** The base side is read from git. The other side is the working tree, so run it on a checkout of the pull request, or locally with uncommitted edits.
- **Exit codes.** 1 when the change introduces an error, or a warning with `--strict`. 0 otherwise. Findings that were already there are counted (`existing_findings`) but never fail the run.
- **Touched nodes.** The changed nodes, plus the nodes that link to them, are listed in the JSON output and become the test cases in the JUnit report.
- **Size of the change.** The report says whether the change is breaking, added or only edited, with the same rules as [releases](format.md#releases).

## Reports

| Option | Format | Use |
|---|---|---|
| (default) | text | the job log |
| `--json` | JSON with `schema_version`, `new_findings`, `fixed_findings`, `touched`, `bump`, `failed` | scripts and agents |
| `--sarif FILE` | SARIF 2.1.0, new findings only | GitHub code scanning, and other tools that read SARIF |
| `--junit FILE` | JUnit XML, one test case per touched node | test dashboards |
| `--markdown FILE` | a pull request comment, starting with `<!-- oxispec-ci -->` | a sticky comment |

Every report is written even when the run fails, so upload steps should run `if: always()`.

## GitHub Actions

An example, not tested in this repository's own CI. Posting the comment is left to the workflow, so the binary never needs network access or a token.

```yaml
name: specs
on: pull_request

permissions:
  contents: read
  pull-requests: write      # for the comment
  security-events: write    # for code scanning

jobs:
  oxispec:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v7
        with:
          fetch-depth: 0
      - run: cargo install --git https://github.com/alexrgb/oxispec oxispec --locked
      - run: |
          oxispec ci --base "origin/${{ github.base_ref }}" \
            --sarif oxispec.sarif --markdown oxispec.md
      - if: always()
        uses: github/codeql-action/upload-sarif@v3
        with:
          sarif_file: oxispec.sarif
      - if: always() && github.event.pull_request.head.repo.full_name == github.repository
        env:
          GH_TOKEN: ${{ github.token }}
        run: gh pr comment "${{ github.event.pull_request.number }}" --body-file oxispec.md --edit-last --create-if-none
```

- Code scanning needs GitHub Advanced Security on private repositories. Leave the SARIF step out if you do not have it.
- Pull requests from forks get a read-only token, so the comment step is skipped for them.
- `--edit-last` updates the last comment written with the same token, so the pull request keeps one comment.
- Installing with `cargo install` is slow. Prebuilt binaries are planned (see the roadmap) and will replace that step.

## Not built yet

A `--changed-files` mode for monorepos with many projects, trace and coverage results (they need the scanners in a later iteration), and posting comments from the binary.
