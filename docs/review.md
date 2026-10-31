# Review of iterations 1 to 14

A review of what is built, how well it is tested, what it does under load, and what to do next. It was made after iteration 14, by reading the coverage numbers, using every command on generated projects, and writing the stress tool described in [stress.md](stress.md).

## Tests

At the time of the review: 219 unit tests in `oxispec-core`, 97 integration tests that run the real binary in `oxispec` (the CLI), and 3 smoke tests of the stress tool. All pass on Linux, macOS and Windows in CI, with a minimum Rust job and a beta job.

Line coverage from `cargo llvm-cov` is **95.6%** overall (13,225 lines, 583 missed). The core library is at 97 to 100% in most files. The weakest files:

| File | Line coverage | What is not covered |
|---|---|---|
| `oxispec-cli/src/agents.rs` | 70.7% | the paths that write and replace skill files |
| `oxispec-cli/src/context.rs` | 84.0% | error paths of `oxispec context` |
| `oxispec-cli/src/machines.rs` | 90.0% | `--at` and some error paths |
| `oxispec-cli/src/rollback.rs` | 91.0% | refusal paths |
| `oxispec-core/src/report.rs` | 90.6% | some SARIF and comment branches |

Coverage says what ran, not that the answers are right. What gives confidence in the answers: the claims solver is tested on many conflict shapes, the generated projects have an oracle (`check` must find exactly the planted conflicts), and the CLI tests check real output, not only exit codes.

## Findings

Fixed in the pull request that added this review:

1. **Concurrent searches broke the search index.** Several `oxispec search` processes starting together on a cold cache deleted each other's files, and 132 of 133 MCP search calls failed in the load test. A file lock now serializes it, and a test covers it.
2. **Concurrent `oxispec new` gave two requirements the same ID.** 26 of 48 requirements collided with four writers. Choosing an ID and writing its file are now under a lock, and a test covers it.
3. **A byte order mark hid a file.** A node file saved with a BOM (Windows editors do this) was silently not a node. It is read now.
4. **A closed pipe printed a panic.** `oxispec init | head` ended with a Rust panic message. It now stops quietly.
5. **The deployment view was quadratic.** Building edges compared every pair of hosted elements and allocated for each, about half a second at 1,500 containers. It uses a lookup now.

Not fixed, and worth knowing:

- **Cold start is slow on a big project and single threaded.** With 12,000 files, a cold `check` takes seconds, mostly waiting on the disk (user time was under a second). Reading files in parallel would help. A warm `check` is well under a second.
- **A state machine block has a size limit.** The YAML parser stops at 250,000 nodes, about 30,000 transitions in one block, and the block is reported as unreadable.
- **Each MCP read call starts a process.** A call costs tens of milliseconds even on a small project. Fine for an agent, wrong for a server. Server mode will need the graph in memory.
- **`site` is the slowest command.** About 1.5 ms per page: 9 to 15 seconds for 12,000 nodes.
- **Other commands that edit files are not locked.** `link`, `change status`, `rollback` and `confirm` read a file and write it back, so two at once on one file can lose an edit. Only `new` is locked now. Git is still the answer for separate working trees.
- **The search lock serializes all persistent searches.** Under 8 parallel readers on a small project the median search took 0.4 s. A shared lock for readers would fix it if it matters.

## Scale

Release build, on a laptop that was busy, so read these as orders of magnitude (the budgets in the tool are about four times a quiet measurement):

| Project | Cold `check` | Warm `check` | `ci --base` | `site` |
|---|---|---|---|---|
| 200 requirements | 0.02 s | 0.01 s | 0.3 s | 0.1 s |
| 2,000 requirements | 0.2 s | 0.06 s | 1 s | 0.5 s |
| 10,000 requirements, 1,000 commits | 3 to 4 s | 0.3 s | 2 to 8 s | 9 to 15 s |

Peak memory stayed under 130 MB on the large project. 3,000 claims on one variable, a chain of 3,000 refining requirements, 400 KB requirements and 20 planted conflicts all stayed fast and correct.

MCP load, 8 clients on 2,000 requirements: about 100 requests per second, p95 around 120 ms, no errors once the search lock was in.

## What to do next

1. Run the `stress` workflow once on `main` and keep its numbers as the baseline. The budgets were set from laptop runs and have not yet run on a CI machine.
2. Add a property test that compares the claims solver with a brute force over small domains. It is the piece where a wrong answer would be silent.
3. Cover the weak files above, starting with `oxispec-cli/src/agents.rs`.
4. Read files in parallel on a cold start, and decide whether the other editing commands need the same lock as `new`.
5. Before server mode, hold the graph in memory so a request does not start a process.
