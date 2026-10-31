# Stress and load tests

`oxispec-stress` is a development tool in this repository (`crates/oxispec-stress`, not published and not part of releases). It makes large synthetic projects, times every command on them, and load-tests the MCP server and concurrent command line use. It tests the `oxispec` program that sits next to it, or the one given with `--bin`.

```bash
cargo build --release -p oxispec -p oxispec-stress
./target/release/oxispec-stress run --preset medium          # time every command, check budgets
./target/release/oxispec-stress load --preset medium         # 8 MCP clients at once
./target/release/oxispec-stress load --preset small --mode cli --clients 8 --writers 4
./target/release/oxispec-stress generate --out /tmp/big --preset large   # keep a project to look at
```

## The projects

`generate` writes a project with requirements and scenarios, links, ADRs, a vocabulary with claims, state machines, an architecture model with an environment, annotated code, and git history with tags `v0` and `v1`. Without `--dir`, `run` and `load` make a project in a temporary folder.

| Preset | Requirements | ADRs | State machines (states) | Containers | Commits |
|---|---|---|---|---|---|
| `tiny` | 30 | 5 | 2 (10) | 8 | 5 |
| `small` | 200 | 20 | 5 (20) | 30 | 50 |
| `medium` | 2,000 | 200 | 20 (50) | 300 | 300 |
| `large` | 10,000 | 1,000 | 50 (100) | 1,500 | 1,000 |

Knobs change one thing at a time: `--requirements`, `--commits`, `--machines`, `--machine-states`, `--containers`, and the pathological ones:

| Knob | What it stresses |
|---|---|
| `--hot-claims N` | N claims on one variable, which is the solver's worst case |
| `--machine-states N` | one very large state machine |
| `--fat-kb N` | N kilobytes of text in every requirement |
| `--chain N` | a chain of N requirements that each refine the last |
| `--conflicts N` | N planted pairs of contradicting claims |

The generator is seeded (`--seed`), so a project is the same every time.

## `run`: every command, timed

Each command runs `--repeat` times (default 3). The median wall time and the peak memory of the process are reported (memory is not measured on Windows). The commands: `check` cold and warm, `check --changed`, `index`, `search`, `trace`, `coverage`, `claims`, `machine`, `architecture` views, `context`, `history`, `diff`, `ci` and `site`.

When it makes the project itself, `run` first checks the answers: `check` must report exactly the planted conflicts and no other error or warning. A fast wrong answer is a failure.

A command over its budget (seconds and megabytes, per preset, in `crates/oxispec-stress/src/run.rs`) fails the run. The budget is compared with the best of the runs, because a busy machine makes a run slower and never faster. The budgets are about four times what a laptop measured, so a slow runner passes and a change that makes something several times slower does not. `--no-budget` reports without failing. `--json FILE` writes the numbers.

## `load`: many at once

- **`--mode mcp`** starts `--clients` MCP servers and sends each `--requests` calls, one after another, in the mix an agent makes: searches, reads of one node, context packs, neighbours, and now and then a whole-project check. It reports calls, errors and p50, p95, p99 and max latency per tool, and requests per second. `--max-p95-ms` and `--max-errors` set the pass line.
- **`--mode cli`** runs `--clients` processes that each run `check`, `search`, `trace`, `coverage` and `claims` in turn on one project, so they share its cache and search index. With `--writers N`, that many processes add requirements at the same time, and afterwards no two may share an ID.

## What it found

The first runs found, and the tests now cover:

- Several `oxispec search` processes starting at the same time on a cold cache corrupted the search index (lock errors, missing files). A file lock now serializes building it.
- `oxispec new` processes at the same time picked the same ID. A lock now covers choosing an ID and writing its file.

## In CI

The `stress` workflow runs every Monday and on request (Actions, `stress`, choose a size). It builds in release mode, runs `run` against the budgets, an MCP load, a command line load with writers, and the pathological shapes. Results are uploaded as JSON.

Tests of the tool itself run with `cargo test`: a `tiny` run with the oracle, a short MCP load, and a command line load with writers.

## Limits to know

- A state machine block is read with a YAML parser budget of about 250,000 nodes, which is roughly 30,000 transitions in one block. Beyond that the block is reported as unreadable (E020).
- Cold start reads files one at a time. On the `large` preset a cold `check` takes seconds, mostly waiting for the disk, and a warm one well under a second.
- Each MCP read tool starts the `oxispec` program once per call, so a call costs tens of milliseconds even for a small project. That is fine for an agent and would not be for a service. Server mode will need the graph held in memory.
