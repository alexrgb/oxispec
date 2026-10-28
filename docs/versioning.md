# Versioning

How OxiSpec numbers things, and the rules for commits and releases. Short version:

- The software uses [Semantic Versioning 2.0.0](https://semver.org). One version for the whole workspace.
- Commit and pull request titles follow [Conventional Commits](https://www.conventionalcommits.org). Releases and the changelog come from them.
- The software version is not the only version. File formats, JSON output, caches, stores, skills and MCP tools each have their own compatibility rule (see below).
- Only release pull requests change version numbers and `CHANGELOG.md`.

## Software version

Format: `MAJOR.MINOR.PATCH`. Every crate in the workspace shares one version, set once in `[workspace.package]` in `Cargo.toml`. The CLI crate `oxispec` and the library crate `oxispec-core` always release together.

**Before 1.0.0** (`0.MINOR.PATCH`):

| Bump | When |
|---|---|
| MINOR | A new feature, or any breaking change |
| PATCH | Bug fixes only |

**From 1.0.0:**

| Bump | When |
|---|---|
| MAJOR | A breaking change to a stable surface (see the table below) |
| MINOR | A backwards compatible feature |
| PATCH | A backwards compatible fix |

Other rules:

- `0.0.x` versions are untagged development versions. The first release is `v0.1.0`.
- **Pre-releases** use `-rc.N`, for example `v0.3.0-rc.1`. They are tagged and published like releases but marked as pre-release.
- **Rust version (MSRV).** `rust-version` in `Cargo.toml` is the oldest supported compiler. Raising it counts as a MINOR bump before 1.0 and is listed in the changelog.
- **Iterations are not versions.** An iteration in [iterations.md](iterations.md) is a unit of planning. A release collects one or more merged iterations.

### What 1.0.0 means

We release 1.0.0 when all of these hold:

1. The project file format is stable and `oxispec migrate` upgrades older formats.
2. The JSON output of `check` and `show` has a stable `schema_version`.
3. Check codes (`E001` and so on) are stable.
4. The deprecation policy below has been used at least once.

## Commits and pull request titles

Pull requests are squash merged, so the **pull request title becomes the commit message** on `main`. It must follow Conventional Commits:

```
type(scope): subject
```

| Type | Use for | Release effect |
|---|---|---|
| `feat` | A user-visible feature | MINOR |
| `fix` | A bug fix | PATCH |
| `perf` | A performance improvement | PATCH |
| `docs` | Documentation only | none |
| `refactor`, `test`, `build`, `ci`, `chore` | Changes with no user-visible effect | none |

- **Breaking changes** add `!` after the type or scope, for example `feat(cli)!: rename --strict to --deny-warnings`, and explain the change in a `BREAKING CHANGE:` footer. Before 1.0 this bumps MINOR, from 1.0 it bumps MAJOR.
- **Scopes** name the area: `core`, `cli`, `docs`, `ci`, and later `index`, `history`, `mcp`, `skills`, `scanners`.
- The subject is lowercase, imperative, and has no trailing period.

Branch names are in [iterations.md](iterations.md): `feat/<n>-<slug>`, `fix/<slug>`, `docs/<slug>`.

## Changelog

`CHANGELOG.md` follows [Keep a Changelog](https://keepachangelog.com): sections **Added**, **Changed**, **Deprecated**, **Removed**, **Fixed**, **Security**, newest release first, with an **Unreleased** section at the top.

- It is written for users, not copied commit by commit.
- It is updated **only in release pull requests**, drafted from the commit titles since the last tag. Feature pull requests do not touch it, which avoids merge conflicts.
- Tooling (for example `git-cliff` or `release-plz`) can generate the draft later. The rules here stay the same.

## Other versions and compatibility rules

The software version tells you about the program. These surfaces have their own rules, because a user can upgrade the program without changing their specs, and the other way round.

| Surface | Version marker | Rule |
|---|---|---|
| **Project format** (`.oxispec/config.yaml` and the layout of spec files) | `format: N` in the config | The tool reads formats `N` and `N-1`. A newer format stops with "this project uses format N, upgrade oxispec". `oxispec migrate` upgrades, is repeatable, and has `--dry-run`. Raising `N` is a breaking change. |
| **Machine output** (`check --json`, `show --json`) | `schema_version` integer in the output | Adding fields does not change it. Removing, renaming or changing the type of a field raises it. Consumers must ignore unknown fields. |
| **Check codes** (`E001`, `W001`) | The code itself | A code is never reused. A changed meaning gets a new code. Removing a check deprecates its code first. |
| **Node IDs** | none | IDs never change and are never reused (design section 4). Content changes are tracked by hash and revision, not by ID. |
| **Local index and cache** | Engine version and index format in the cache key | A mismatch triggers a rebuild. Caches are never migrated. |
| **Revision store** (SQLite or Postgres) | Schema version (`PRAGMA user_version`, or a migrations table) | Migrations only go forward and run automatically after a backup copy. A store newer than the program is refused. |
| **Generated skills** | `metadata.version` and a content hash in each `SKILL.md` | The version changes when the skill's behavior changes. `oxispec update` regenerates. The hash detects hand edits. |
| **MCP server** | `serverInfo.version` is the software version | Tool names are stable. Adding optional inputs or output fields is fine. A breaking change gets a new tool name, and the old one stays through the deprecation period. |

## Deprecation

- A command, flag, JSON field, check code or MCP tool is marked deprecated in a MINOR release. It keeps working and prints a warning, and the changelog says what replaces it.
- **Before 1.0**, it can be removed in the next MINOR release.
- **From 1.0**, it is removed no earlier than the next MAJOR release.

## Releases

Done by hand for now. Automation is a later iteration and does not change these rules.

1. All wanted pull requests are merged and CI on `main` is green.
2. Create `release/vX.Y.Z` from `main`. Set the version in `[workspace.package]`, move **Unreleased** to a dated section in `CHANGELOG.md`, and open a pull request titled `chore(release): vX.Y.Z`.
3. After the squash merge, create an annotated tag on that commit and push it:

   ```bash
   git tag -a vX.Y.Z -m "vX.Y.Z"
   git push origin vX.Y.Z
   ```

4. Publish a GitHub Release from the tag, with the changelog section as the notes. Binaries and crates.io publishing come later.
5. **Hotfix:** branch `release/X.Y.x` from the tag, apply the fix, and release a PATCH.

## Versions of the specs you ship

The releases of your own specs (design section 7) use SemVer too, so teams that depend on a shared spec can pin `^2.3`.

| Bump | Examples |
|---|---|
| MAJOR | A requirement is removed. A claim is tightened or changed so existing implementations may break. An API operation changes incompatibly. A state is removed from a state machine |
| MINOR | A requirement or scenario is added. An optional API field is added. A state is added |
| PATCH | Wording, typos, links. No change to claims, scenarios or contracts |

`oxispec release prepare` suggests the bump from the graph diff and a person confirms. Spec release tags use their own prefix, `specs-v2.3.0`, so they never collide with the software tags `v0.3.0` in the same repository.
