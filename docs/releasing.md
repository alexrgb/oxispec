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
2. **crates.io.** Sign in at crates.io with GitHub and make sure the email of the account is verified (publishing needs it). Open Account Settings, API Tokens, and create a token with the scopes `publish-new` and `publish-update`, and an expiry you will remember to renew. Then store it, pasting the token when `gh` asks for it:

   ```bash
   gh secret set CARGO_REGISTRY_TOKEN --repo alexrgb/oxispec
   ```

   The names `oxispec` and `oxispec-core` were free on 2026-11-08. The first publish claims them, so check again on the day.
3. **npm.** Create an account at npmjs.com and turn on two-factor authentication. Create an access token that can publish packages **without a one-time code**, because the workflow cannot type one (npm calls this an *automation* token, or a granular token with write access that bypasses two-factor authentication when publishing). The package `oxispec` does not exist yet, so the token has to be allowed to publish new packages, not only existing ones. The pages of npm have changed more than once, so use the wording on the screen as the guide. Then:

   ```bash
   gh secret set NPM_TOKEN --repo alexrgb/oxispec
   ```

   The name `oxispec` was free on 2026-11-08. A scoped name such as `@oxispec/oxispec` needs an npm organization, and then `npm-scope = "@oxispec"` goes in `dist-workspace.toml`.
4. **Homebrew.** Create the repository `alexrgb/homebrew-tap` on GitHub, public, empty or with a README. Then create a fine-grained personal access token (Settings, Developer settings, Personal access tokens, Fine-grained tokens) whose resource owner is `alexrgb`, with access to **only** that repository and the permission *Contents: Read and write*. Give it an expiry. Store it:

   ```bash
   gh secret set HOMEBREW_TAP_TOKEN --repo alexrgb/oxispec
   ```

   The formula goes into the folder `Formula` of the tap, and users install with `brew install alexrgb/tap/oxispec`.
5. **Repository settings.** In Settings, Actions, General, set the workflow permissions to read and write, so the workflow can create the release. In Settings, Rules, add a ruleset for the tags that match `v*` that restricts who can create, move and delete them, so only you can make a release.

   A check that the secrets are there: `gh api repos/alexrgb/oxispec/actions/secrets --jq '[.secrets[].name]'` lists their names (never their values).

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
| Tag protection for `v*` | Not checked | Settings, Rules |

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
