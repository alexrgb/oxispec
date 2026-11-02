# Benchmarks

Numbers to compare against when something changes. Every row is the **median of 7 runs** of `oxispec-stress load`, because one run is noisy: on a laptop the p50 of 16 clients moved between 5 and 60 ms from run to run on the same build. A change smaller than that spread is not a result.

```bash
cargo build --release -p oxispec -p oxispec-stress
./target/release/oxispec-stress generate --out /tmp/large --preset large     # once; 12,000 nodes
./target/release/oxispec-stress load --dir /tmp/large --preset large --mode http --clients 16 --requests 100 --repeat 7
```

The mix is the one an agent makes (searches, reads of one node, context packs, neighbours, now and then a whole-project check), described in [stress.md](stress.md). All runs had 0 errors.

## Local, `large` project

Apple M1 Pro, 10 cores, release build. The load generator runs on the same machine as the server, so both compete for the cores.

| Date | Change | Scenario | p50 | p95 | p99 | req/s |
|---|---|---|---|---|---|---|
| 2026-11-02 | before: graph copied on every call | `http`, 16 clients | 131.9 ms | 159.5 ms | 229.3 ms | 112 |
| | | `http`, 1 client | 13.1 ms | 25.9 ms | 37.4 ms | 66 |
| | | `mcp`, 16 processes | 35.0 ms | 125.5 ms | 487.4 ms | 287 |
| 2026-11-02 | after: graph shared, not copied | `http`, 16 clients | 10.4 ms | 33.2 ms | 83.7 ms | 810 |
| | | `http`, 1 client | 1.8 ms | 13.8 ms | 27.5 ms | 251 |
| | | `mcp`, 16 processes | 3.4 ms | 92.2 ms | 426.4 ms | 638 |

## What the first row shows

Before the change a call took a copy of the whole graph while holding the lock that guards it. Sixteen clients queued on that lock, so every request, even a cached `spec_get`, cost about 130 ms: the copy time (about 8 ms for 12,000 nodes) times the number of clients ahead in the queue. The same call alone took 13 ms. Sharing the nodes, findings and ID index between clones removed both the copy and the queue.

## CI runner

The `stress` workflow, large preset, 16 clients, 100 requests each, on a GitHub-hosted Linux runner (2 vCPU for a private repository). Since 2026-11-02 the workflow reports the median of 5 runs; the older rows are single runs, which varied by about 20% on the same code.

| Date | Change | Scenario | p50 | p95 | p99 | req/s |
|---|---|---|---|---|---|---|
| 2026-11-01 | before (single run) | `http` | 506.7 ms | 610.2 ms | 891.3 ms | 23.7 |
| | | `mcp`, 16 processes | 698.4 ms | 1267.5 ms | 4687.8 ms | 19.1 |
| 2026-11-02 | after, median of 5 | `http` | 89.0 ms | 120.0 ms | 154.0 ms | 44.7 |
| | | `mcp`, 16 processes | 344.5 ms | 586.7 ms | 1792.7 ms | 40.6 |

One of the five `http` runs had p50 299 ms; the other four were between 87 and 103 ms. That is why the median is used.

On this runner the gain is smaller than on the laptop (p50 507 to 89 ms for `http`, against 132 to 10 ms locally). Two cores are shared by 16 clients and the server, so the CPU is the limit and the lock is no longer the main cost.

## Not measured yet

These numbers come from a laptop and a shared 2-vCPU runner. A dedicated server, with the load generator on another machine, would show what a team would see. Until then treat the absolute figures as relative to each other, not as a promise.
