# Releasing

A release is a git tag. Pushing a tag that looks like `v0.1.0` starts the `Release` workflow, which builds the program for each platform, creates a GitHub Release, and publishes it to every channel. The rules for version numbers are in [versioning.md](versioning.md).

| Channel | What users run | Made by |
|---|---|---|
| GitHub Release | download an archive for their platform, with a checksum and build provenance | [dist](https://github.com/axodotdev/cargo-dist) |
| Shell and PowerShell installers | `curl -LsSf <url>/oxispec-installer.sh \| sh`, or the `.ps1` on Windows | dist |
| Homebrew | `brew install alexrgb/tap/oxispec` | dist, into the tap repository |
| npm | `npm install -g oxispec`, or `npx oxispec` | dist |
| crates.io | `cargo install oxispec` | the `publish-crates` workflow |

Platforms: macOS on Apple silicon and Intel, Linux on x86_64 and arm64 (glibc), and Windows on x86_64. The configuration is in `dist-workspace.toml`. `release.yml` is generated from it, so change the file and run `dist generate`, never edit the workflow by hand. The `dist` job in CI fails when they disagree.

## Making a release

1. Open a **release pull request** from a branch named `release/vX.Y.Z`. Only a release pull request changes these:
   - `version` in `[workspace.package]` of `Cargo.toml`;
   - the version in `oxispec-core = { ..., version = "=X.Y.Z" }` in `[workspace.dependencies]`, which must be the same number (a test fails when they differ);
   - `Cargo.lock`, which `cargo build` updates;
   - `CHANGELOG.md`: move the entries from `[Unreleased]` under the new version and date.
2. Merge it when CI is green.
3. Tag the merge commit and push the tag:

   ```bash
   git tag -a v0.1.0 -m "v0.1.0"
   git push origin v0.1.0
   ```

4. Watch the `Release` run. It builds, creates the GitHub Release with the notes from the changelog, and then publishes. If one publish job fails, fix the cause and re-run the failed jobs. The crates job skips a version that is already on crates.io.

A version with a suffix such as `v0.2.0-rc.1` is marked as a pre-release on GitHub, and is not published to crates.io. Whether Homebrew and npm get it is decided by dist.

You can try the whole build without publishing: `dist plan` shows what a tag would make, and `dist build --artifacts=local` builds the archives for this machine into `target/distrib/`.

## One-time setup

Do this before the first tag. The workflow fails, with a message that names the missing piece, until it is done.

1. **Make the repository public**, or accept that releases stay private. Build provenance (`github-attestations`) needs a public repository or a plan that supports it for private ones. *Done: the repository is public.*
2. **crates.io:** sign in with GitHub, create an API token that is allowed only to publish new crates and versions, and store it as the repository secret `CARGO_REGISTRY_TOKEN`. The names `oxispec` and `oxispec-core` were free when this was written. The first publish claims them, so check again.
3. **npm:** create an account with two-factor authentication, create an *automation* token that can publish, and store it as the secret `NPM_TOKEN`. The name `oxispec` was free when this was written. A scoped name such as `@oxispec/oxispec` needs an npm organization, and then `npm-scope = "@oxispec"` goes in `dist-workspace.toml`.
4. **Homebrew:** create the repository `alexrgb/homebrew-tap` (empty is fine), and a fine-grained personal access token with write access to its contents only. Store it as the secret `HOMEBREW_TAP_TOKEN`.
5. In the repository settings, allow GitHub Actions to create releases (workflow permissions: read and write), and protect tags that match `v*` so only maintainers can create them.

Tokens are secrets of the repository, never of a pull request from a fork, and the release workflow only runs for tags.

### Where it stands

Checked on 2026-11-08, before the first release. Repeat the checks on the day, since a name can be taken in between.

| Piece | State | How to check |
|---|---|---|
| Repository public | Done | `gh repo view alexrgb/oxispec --json visibility` |
| `oxispec` and `oxispec-core` on crates.io | Both names free | `curl -s -o /dev/null -w '%{http_code}' https://crates.io/api/v1/crates/oxispec` (404 means free) |
| `oxispec` on npm | Free | `curl -s -o /dev/null -w '%{http_code}' https://registry.npmjs.org/oxispec` (404 means free) |
| Secret `CARGO_REGISTRY_TOKEN` | **Not set** | `gh api repos/alexrgb/oxispec/actions/secrets --jq '[.secrets[].name]'` |
| Secret `NPM_TOKEN` | **Not set** | the same |
| Secret `HOMEBREW_TAP_TOKEN` | **Not set** | the same |
| Repository `alexrgb/homebrew-tap` | **Does not exist** | `gh repo view alexrgb/homebrew-tap` |
| Tag protection for `v*` | Not checked | repository settings, Rules |

## Before the first tag

1. The setup above is done, and the three secrets are listed.
2. The last pull request on `main` has a green CI, including the `dist` and `package` jobs.
3. `cargo publish --dry-run -p oxispec-core --locked` passes. The same for `oxispec` can only pass once `oxispec-core` is on crates.io, because it depends on that exact version: it is the reason the crates are published in that order.
4. `dist plan` shows the five platforms and the four installers.
5. The release pull request (`release/v0.1.0`) is merged: the changelog has the version and the date of the day the tag is made, and `Cargo.toml` and `Cargo.lock` agree.
6. A person tags the merge commit. Nothing here tags for you.

## When something goes wrong

- **A bad release.** Do not delete and re-tag. Fix forward with a new patch version. Yank the bad one with `cargo yank --version X.Y.Z oxispec` (and `oxispec-core`), `npm deprecate oxispec@X.Y.Z "reason"`, and edit the GitHub Release to say so. Homebrew users get the next formula.
- **A failed publish job.** Re-run the failed jobs from the Actions page. Nothing is published twice: crates.io refuses a repeat and the job checks first, npm refuses a repeat version, and the tap commit is replaced.
- **A change to the build.** Edit `dist-workspace.toml`, run `dist generate`, and commit both. The `dist` job in CI and the `Release` workflow (which runs `dist plan` on every pull request) fail on a broken config before a tag.
- **A new platform.** Add its target triple to `targets`. Check first that the C code in the dependencies (tree-sitter) builds there.

## What is not done yet

- Linux builds with musl, for static binaries.
- Signed and notarized macOS binaries, and Windows code signing. Until then macOS may ask the user to allow the first run when the program was downloaded with a browser. The shell installer and Homebrew do not trigger that.
- A winget package and a Debian or RPM package.
- Automating the release pull request from Conventional Commits (for example with release-plz). It is a manual pull request now.
