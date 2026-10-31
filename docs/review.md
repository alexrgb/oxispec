# Review of iterations 1 to 14

A review of what is built, how well it is tested, what it does under load, and what to do next. It was made after iteration 14, by reading the coverage numbers, using every command on generated projects, and writing the stress tool described in [stress.md](stress.md).

## Tests

At the time of the review: 219 unit tests in `oxispec-core`, 97 integration tests that run the real binary in `oxispec` (the CLI), and 3 smoke tests of the stress tool. All pass on Linux, macOS and Windows in CI, with a minimum Rust job and a beta job.

Line coverage from `cargo llvm-cov` is **95.6%** overall (13,225 lines, 583 missed). The core library is at 97 to 100% in most files. The weakest files:

| File | Line coverage | What is not covered |
|---|---|---|
| `oxispec-cli/src/agents.rs` | 70.7% at review time | `agents list`, bad client names and drift reports, now tested |
| `oxispec-cli/src/context.rs` | 84.0% at review time | pointers when the budget is small, now tested |
| `oxispec-cli/src/machines.rs` | 90.0% | `--at` and some error paths |
| `oxispec-cli/src/rollback.rs` | 91.0% | refusal paths |
| `oxispec-core/src/report.rs` | 90.6% | some SARIF and comment branches |

Coverage says what ran, not that the answers are right. A property test compares the claims solver with a brute force over every role and level for random claims (600 cases in the suite, and 20,000 in one run with no disagreement). What else gives confidence in the answers: the claims solver is tested on many conflict shapes, the generated projects have an oracle (`check` must find exactly the planted conflicts), and the CLI tests check real output, not only exit codes.

## Findings

Fixed in the pull requests that added this review and the stress tool:

1. **Concurrent searches broke the search index.** Several `oxispec search` processes starting together on a cold cache deleted each other's files, and 132 of 133 MCP search calls failed in the load test. A file lock now serializes it, and a test covers it.
2. **Concurrent `oxispec new` gave two requirements the same ID.** 26 of 48 requirements collided with four writers. Choosing an ID and writing its file are now under a lock, and a test covers it.
3. **A byte order mark hid a file.** A node file saved with a BOM (Windows editors do this) was silently not a node. It is read now.
4. **A closed pipe printed a panic.** `oxispec init | head` ended with a Rust panic message. It now stops quietly.
5. **The deployment view was quadratic.** Building edges compared every pair of hosted elements and allocated for each, about half a second at 1,500 containers. It uses a lookup now.

6. **`machine --tests` ran out of memory on a big machine.** It built every test's whole path before printing, so memory grew with transitions times depth, and the CI runner killed it on a 10,000-state machine. The tests are made one at a time now, there is a `--limit`, and the MCP tool returns at most 200. The stress workflow on `main` found this one, on its first run.
7. **Commands that edit a file could lose each other's edits.** Eight `oxispec link` runs at once on one requirement kept 5 of 8 links. Every command that writes (`new`, `link`, `change`, `confirm`, `results`, `release`, `rollback`) now takes one project lock first, and a test covers it.
8. **Cold start was single threaded.** With 12,000 files a cold `check` took 3 to 4 seconds on the laptop, mostly waiting for the disk. Files are now read, hashed and parsed on up to 8 threads, which took a cold `index` from 4.3 s to 1.4 s and from 0.73 s to 0.34 s on two runs. Results do not depend on the thread count, and a test covers it.

Not fixed, and worth knowing:

- **A state machine block has a size limit.** The YAML parser stops at 250,000 nodes, about 30,000 transitions in one block, and the block is reported as unreadable.
- **Each MCP read call started a process. Fixed in iteration 18.** Tools now run in the server's own process, and the server keeps the last graph in memory until a markdown file changes. On the 2,000-requirement project (8 clients, a busy laptop) that took the load from about 80 to about 180 requests per second and the median from 73 to 36 ms. A file watcher then removed the per-call walk of the project, and the result of the checks is kept with the graph. On the 12,000-file project one server answers `spec_get`, `spec_check` and `spec_status` in about 10 ms (from 44, 180 and 190 ms), and `spec_search` in about 27 ms (from 62). `spec_claims` still takes about 110 ms, since its analysis is not kept.
- **`site` is the slowest command.** About 1.5 ms per page: 9 to 15 seconds for 12,000 nodes.
- **The search lock serializes all persistent searches.** Under 8 parallel readers on a small project the median search took 0.4 s. A shared lock for readers would fix it if it matters.

## Scale

Release build, on a laptop that was busy, so read these as orders of magnitude (the budgets in the tool are about four times a quiet measurement):

| Project | Cold `check` | Warm `check` | `ci --base` | `site` |
|---|---|---|---|---|
| 200 requirements | 0.02 s | 0.01 s | 0.3 s | 0.1 s |
| 2,000 requirements | 0.2 s | 0.06 s | 1 s | 0.5 s |
| 10,000 requirements, 1,000 commits | 3 to 4 s | 0.3 s | 2 to 8 s | 9 to 15 s |

Peak memory stayed under 130 MB on the large project. 3,000 claims on one variable, a chain of 3,000 refining requirements, 400 KB requirements and 20 planted conflicts all stayed fast and correct.

MCP load, 8 clients on 2,000 requirements: about 100 requests per second, p95 around 120 ms, no errors once the search lock was in. On the large project, 16 clients on a CI runner: 6.4 requests per second and p95 4.3 s, because every call starts a process that loads 12,000 nodes. This is the cost of one process per call, in numbers.

## What to do next

1. Tighten the budgets further. The first run on a CI machine (large preset, Linux) was well inside them. The budgets for cold `check`, `index` and `site` on `large` are now 3 s, 3 s and 5 s, and the others are still about four times a laptop measurement.
2. For server mode, watch files instead of walking them on every call.
