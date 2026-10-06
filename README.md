# rust-lab

The goal is a **fast Rust kernel for JupyterLab**, with Python-like interactive
cell response and native Rust execution. The CLI is a development and benchmark
harness for the reusable engine. See [ROADMAP.md](ROADMAP.md) for the notebook
milestones and the lessons drawn from Evcxr.

Phase 1 establishes a measurable execution baseline.
This release deliberately uses normal Cargo/rustc builds and native child
processes. It does **not** yet claim lower latency than Evcxr or Python.
There is no Jupyter integration, shared-library loader, or JIT in this phase.

Requires Rust **1.99.0 or newer**, including Cargo, rustfmt, and Clippy.
The workspace has no third-party dependencies and builds offline.
Licensed under Apache-2.0; see [LICENSE](LICENSE).

## Quick start

```sh
cargo build --workspace --offline
cargo run --offline -p rust-lab-cli -- --expr '1 + 2'
cargo run --offline -p rust-lab-cli -- --eval 'let x = 10; println!("{x}");'
cargo run --offline -p rust-lab-cli -- --file examples/cell.rs
cargo run --offline -p rust-lab-cli
```

Example interactive session:

```text
rust> let mut x = 10;
rust> x += 2;
rust> :expr x + 20
32
rust> :begin
... fn twice(value: i32) -> i32 {
...     value * 2
... }
... let y = twice(x);
... :end
rust> :expr y
24
rust> :reset
rust> :quit
```

Plain lines are snippets: supply semicolons as normal Rust requires. `:expr`
evaluates and Debug-prints an expression. `:begin` / `:end` submit a multiline
snippet as one cell. `:help` lists commands. Empty cells and unknown commands
are rejected. End-of-file exits; an unfinished multiline cell reports an error.
Piped lines use the same session protocol without prompts. To read an entire
multiline snippet instead, use `--file -`:

```sh
printf 'let x = 10;\n:expr x + 20\n:quit\n' | target/debug/rust-lab
cat examples/cell.rs | target/debug/rust-lab --file -
```

## Architecture

```text
rust-lab-cli        input, REPL commands, benchmark CSV
      |
Session<B>         retained source history, commit/reset, stage timings
      |
source generation  wrap prior snippets and current cell in fn main
      |
Backend            prepare -> compile -> execute (associated handle types)
      |
CargoBackend       private temporary crate + per-session target cache
                   cargo build --offline -> executable -> child process
```

`rust-lab-engine` contains `session`, `backend`, `timing`, and `error` modules.
The generic backend interface owns compilation and execution resources. A later
backend can return compiler IR and JIT handles without exposing Cargo paths to
the session or CLI. Session semantics and source generation will also need to
evolve when live values replace replay. No compiler-internal APIs are assumed.

Each session has an independent temporary crate and incremental build cache.
The backend deletes its own temporary directory when dropped normally. An
abruptly killed engine can leave `rust-lab-*` directories in the system temp
directory. Generated cells run in that temporary directory, with stdin closed.
Compiler diagnostics refer to generated source, rather than mapped cell lines.
Standard-library snippets, statements, and local items work; dependency
management and whole-crate syntax such as crate attributes are outside Phase 1.

### Session semantics and limits

Successful snippets are kept in memory for the lifetime of the CLI session.
Every subsequent cell recompiles and **replays all retained snippets** in one
`main` scope. This reconstructs bindings, mutation, shadowing, and functions;
it does not preserve heap allocations or runtime values between child processes.
Prior prints, I/O, and other side effects repeat, and replay cost grows with
history length. Nondeterministic operations may produce different values.

Expressions are observations and are not added to history. An expression's
mutations, moves, or side effects are therefore not retained as session source.
Use a snippet for changes you want subsequent cells to reconstruct. Session
history is committed only after successful compilation and a successful child
exit. Errors and panics leave source history unchanged; already-performed
external effects cannot be rolled back. `:reset` clears history and retains the
backend's cache. Sessions are not saved across CLI restarts.

Cells execute ordinary native code with the user's permissions. This is a tool
for trusted local code, not a sandbox. Input, output, and execution time are
unbounded in this initial baseline; interactive stdin inside cells is unsupported.

## Timing and benchmarks

Normal evaluations print cell stdout to stdout and diagnostics/timings to stderr.
Successful and failed child executions have timings; a preparation or compilation
error reports its stage and diagnostics without a complete timing record.

| Measurement | Included work |
| --- | --- |
| `session_setup_ms` | Allocate session temp crate; reported separately |
| `source_generation_ms` | Assemble prior snippets and current cell |
| `prepare_ms` | Compare/write generated source |
| `compile_link_ms` | Cargo process, fingerprint checks, rustc, and linking |
| `execute_process_ms` | Child startup, replay, execution, stdout/stderr capture |
| `total_ms` | End-to-end evaluation, including small orchestration/commit overhead |

Times are wall-clock measurements using `Instant`. They do not isolate parsing,
borrow checking, LLVM code generation, linking, or pure user-code runtime.
Total excludes CLI startup, argument parsing, session setup, and report printing.
All cell builds use the dev profile, even when the CLI itself is a release build.

Build the CLI before collecting samples, so its own build is outside measurement:

```sh
cargo build --release --offline -p rust-lab-cli
mkdir -p benchmarks/results

# One first-build sample followed by source changes in the same session.
target/release/rust-lab bench --mode changed --iterations 20 --warmup 0 \
  > benchmarks/results/first-and-changed.csv 2> benchmarks/results/first-and-changed.log

# Warm incremental compilation: source changes on every measured iteration.
target/release/rust-lab bench --mode changed --iterations 20 --warmup 2 \
  > benchmarks/results/changed.csv 2> benchmarks/results/changed.log

# Cargo fingerprint/cache-hit cost, plus running an already-built executable.
target/release/rust-lab bench --mode cached --iterations 20 --warmup 2 \
  > benchmarks/results/cached.csv 2> benchmarks/results/cached.log

# A different workload; std::hint::black_box wraps the expression's result.
target/release/rust-lab bench --expr '(0u64..10_000).sum::<u64>()' \
  --iterations 20 --warmup 2 > benchmarks/results/sum.csv
```

CSV rows contain a condition, sample number, and all five cell timing stages.
`first-build` means a fresh session target directory, not a cold operating-system
filesystem cache. `source-change` inserts a unique source comment to force Cargo
to invoke rustc while allowing its normal incremental cache. `cache-hit` preserves
identical source and its modification time. These benchmark expressions do not
grow session history. Warmups are executed but excluded from samples. Captured
cell output is discarded in benchmark mode; failures stop the benchmark with a
nonzero exit status. Logs include Cargo/rustc versions, platform, benchmark mode,
setup time, and min/p50/p95/max total latency using nearest-rank percentiles.

Keep raw CSV and metadata together. For comparisons, record CPU, workload,
power settings, load, and compiler environment (e.g. RUSTFLAGS and wrappers).
Use the same workload/profile and compare like-for-like conditions. Cached
results are not evidence of compiler speed, and this project currently has no
measured Evcxr or Python comparison. Persistent compiler/Cranelift latency goals
from early discussions remain targets rather than measured improvements.


Initial measurements and raw samples: [Phase 1 baseline](benchmarks/initial-baseline.md).

## Development checks

```sh
cargo fmt --all
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --offline -- -D warnings
cargo test --workspace --offline
```

Tests cover source-history commits and rollback, reset, expression observations,
input validation, real Cargo execution, session isolation, multiline code,
compiler-error recovery, panic recovery, temp cleanup, and CLI/CSV behavior.
The real-backend tests require Cargo/rustc on PATH and execute generated programs.

Next work focuses on notebook cell semantics, a persistent execution worker,
and a thin Jupyter adapter around the engine. Compiler/dependency caching and
Cranelift experiments will be judged using both engine timings and real notebook
round trips. JupyterLab is the intended user interface; Phase 1 provides the
baseline for that work.
