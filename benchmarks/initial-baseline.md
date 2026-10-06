# Phase 1 baseline — October 6, 2026

Measured on Ubuntu in WSL2, x86_64, with an Intel Core i5-10210U
(4 cores / 8 logical processors). Rustc 1.99.0 (b940084d7) and
Cargo 1.99.0 (5f94df478). The CLI was built in release mode; generated cells
used Cargo's dev profile. Workload: `1u64 + 2`, wrapped in `std::hint::black_box`.

| Condition | Measured samples | Warmups | Total p50 (ms) | Total p95 (ms) |
| --- | ---: | ---: | ---: | ---: |
| First build in a fresh session | 1 | 0 | 267.463 | 267.463 |
| Warm source change | 20 | 2 | 245.172 | 287.991 |
| Identical-source Cargo cache hit | 20 | 2 | 16.960 | 18.517 |

The first-build observation is sample 1 of `first-and-changed.csv`; its other
nine samples are source changes and are not pooled with the first-build value.
One observation is not a reliable first-build latency distribution.
Warm source changes alter a source comment, forcing a rustc invocation while
allowing incremental reuse of semantically identical code. This is a minimal
source-revision baseline, not a measurement of compiling newly added logic.
Cached mode avoids rewriting source and still starts Cargo and the executable.

Raw CSV, stderr metadata, and machine/source metadata are preserved under
[`baseline-2026-10-06/`](baseline-2026-10-06/). Percentiles use nearest rank.
CSV reports source generation, source preparation, compile/link, process
execution, and total wall time for each evaluation. Compilation dominates
the source-change measurements; process startup, capture, and this tiny
computation together usually take less than 1 ms.

Commands, after `cargo build --release --offline -p rust-lab-cli`:

```sh
target/release/rust-lab bench --mode changed --iterations 10 --warmup 0
target/release/rust-lab bench --mode changed --iterations 20 --warmup 2
target/release/rust-lab bench --mode cached --iterations 20 --warmup 2
```

Filesystem caches, system load, CPU frequency, and power settings were not
controlled. No Python, Evcxr, Cranelift, or JIT benchmark was run. These results
characterize this baseline on this machine and establish no cross-engine
speedup. Future comparisons should repeat the commands under matched conditions
and add realistic workloads and growing session histories.

Validation on Rust 1.99.0: `cargo fmt --all -- --check`,
`cargo clippy --workspace --all-targets --offline -- -D warnings`, and
`cargo test --workspace --offline` passed (13 tests, including actual generated
Cargo programs and end-to-end CLI/CSV checks).
