# Contributing to OxiSpec

Thanks for your interest. OxiSpec is early, so the most useful contributions right now are bug reports, feedback on the [design](docs/design.md), and small, focused pull requests.

## Before you start

- Read the [design](docs/design.md) and the [iteration plan](docs/iterations.md). Work is organized in iterations, and each one has a "done when" line.
- For anything bigger than a small fix, open an issue first so we can agree on the approach before you spend time on it.
- Security problems are not public issues. See [SECURITY.md](SECURITY.md).

## Set up

You need a recent stable Rust toolchain. The oldest supported version is `rust-version` in `Cargo.toml`. You also need a C compiler (Xcode command line tools, `build-essential`, or MSVC), because the tree-sitter parsers are built from C source.

```bash
cargo build
cargo test --workspace
```

These are the same checks CI runs. Run all three before you push:

```bash
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

## Making a change

1. Branch from `main`. Names: `feat/<n>-<slug>` for an iteration, `fix/<slug>`, `docs/<slug>`, `ci/<slug>`.
2. Keep the change small and focused. A reviewer should be able to read the whole diff.
3. Add tests for new behavior. Fix the test first for a bug fix when you can.
4. Update the docs when behavior changes. If a design decision changes, update `docs/design.md` in the same pull request.
5. Open a pull request. The **title must follow [Conventional Commits](https://www.conventionalcommits.org)**, for example `feat(cli): add oxispec link`, because pull requests are squash merged and the title becomes the commit message. See [docs/versioning.md](docs/versioning.md).
6. Do not change version numbers or `CHANGELOG.md`. Release pull requests do that, see [docs/releasing.md](docs/releasing.md).
7. If you change a skill in `crates/oxispec-core/skills/`, regenerate the published folder with `cargo run -- agents export --to skills-repo --all`, and commit it. A test fails when `skills/` is out of date. A skill may only name commands and flags that exist: a test checks that too.

CI must pass before a pull request is merged. Performance and load checks on large generated projects are described in [docs/stress.md](docs/stress.md).

## Code style

- `cargo fmt` and clippy with warnings denied are the style guide.
- Keep the core crate (`oxispec-core`) free of terminal and network code. The CLI and other front ends are thin layers over it.
- Prefer clear error messages that say what to do next.
- Comments explain why, not what.

## AI-assisted contributions

Using an AI assistant is fine. You are responsible for what you submit: read it, understand it, and run the tests. A pull request that you cannot explain will be closed.

## Licensing

OxiSpec is licensed under either of [Apache License 2.0](LICENSE-APACHE) or [MIT license](LICENSE-MIT), at your option.

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in this work by you, as defined in the Apache-2.0 license, shall be dual licensed as above, without any additional terms or conditions. There is no CLA.
