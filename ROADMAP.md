# Fast Rust kernel for JupyterLab

The product goal is Rust notebooks that respond like Python notebooks while
executing native Rust code. JupyterLab is the intended interface. The existing
terminal CLI lets us test and benchmark the engine during development.

## Phase 1: execution baseline — complete

The workspace separates session semantics, execution backends, and the CLI.
It accepts Rust code, measures stages, reports compiler/runtime failures, and
includes 13 tests. See [the initial measurements](benchmarks/initial-baseline.md).

The measured source-comment revision median is about 245 ms, and an identical
source Cargo cache hit is about 17 ms on the recorded WSL machine. Neither
measurement represents a Jupyter cell round trip. The source revision retains
semantically identical code, so it also does not measure compiling new logic.
Replay-based state is a temporary baseline. It repeats prior side effects and
cannot serve as the final notebook state model.

## Next engine milestone: notebook cell semantics and live state

Keep a persistent execution worker that owns values between cells. Maintain
source metadata for imports, functions, structs, and variable types separately
from runtime values. Use a Rust parser/analyzer to distinguish statements,
items, and trailing expressions; notebook users should enter ordinary Rust
cells without the CLI's `:expr` protocol.

Acceptance checks:

- A value created once stays alive across later cells; allocation and I/O in
  a previous cell do not repeat when the next cell runs.
- Mutations and ownership moves in executed expressions affect later cells.
- Compile errors leave committed session metadata and live values usable.
- Panic/crash handling reports which values remain available; it must not
  promise rollback of mutations or external effects.
- Diagnostics point to the original cell, with generated code mapped separately.
- Define supported types and lifetimes explicitly, including limits on borrowed
  values, closures, user-defined type changes, and non-Send values.
- Test worker interruption, restart, and resource/code lifetime management.

A shared-library backend can serve as an intermediate implementation while
the execution interface remains replaceable. Keep the Phase 1 backend for
comparison. A persistent worker alone will not eliminate compiler latency.

## Notebook milestone: thin Jupyter kernel adapter

Add a separate kernel crate that talks directly to the engine. Keep compilation,
session state, and timing out of the transport implementation. Implement connection
file handling, message framing/signing, kernel installation, and the shell,
control, heartbeat, IOPub, and stdin channels needed by the supported requests.

Start with kernel information, cell execution, execution counts, stream output,
expression results, error output, busy/idle status, interruption, shutdown, and
restart. Preserve request identities and parent headers. Define handling for
silent requests, history, and unsupported stdin before claiming compatibility.
Long execution must leave heartbeat and interrupt/control handling responsive.
Then add MIME displays, completion, inspection, and input support.

Acceptance checks:

- Install the Rust kernel and select it in JupyterLab.
- Execute multiline cells and trailing expressions with persistent live state.
- See stdout/stderr, results, and cell diagnostics in the correct output area.
- Interrupt a long-running cell and run another cell; restart clears state.
- Exercise message ordering and request/reply behavior through `jupyter_client`
  tests, and verify an actual JupyterLab notebook.

An adapter can be prototyped alongside live-state work, but replay behavior must
stay clearly identified as experimental until the engine milestone passes.

## Performance milestone: reduce the compile/execute hot path

Measure changed logic, repeated function calls, new types, generic-heavy cells,
dependency additions, and growing notebooks. Track first-build, changed-code,
and cache-hit conditions separately. Add frontend, code-generation, linking,
loading, and runtime timings as the backend exposes those stages.

Cache dependency artifacts and unchanged functions. Investigate removing Cargo
from ordinary cell execution, retaining compiler state, and using Cranelift
for fast code generation/in-memory execution. Each experiment needs an explicit
supported Rust/toolchain scope. A Cranelift machine-code backend still requires
a Rust frontend for typing, ownership, generics, and language correctness;
full Rust cannot be replaced by a small expression parser.

Use an initial engineering target of under 50 ms for simple warm cells and under
100 ms at p95 for a defined small-cell suite. These are targets, not guarantees.
Measure kernel round trips as well as engine work, and compare with IPython on
the same machine/client with matched outputs and warm/cold conditions. Separate
interactive latency from numerical execution throughput.

## Evcxr ideas reviewed

The supplied `evcxr-main.zip` was inspected as reference material, along with
the earlier "Detailed Repository Summary" discussion. Relevant source areas:

| Archive path | Idea to carry forward |
| --- | --- |
| `evcxr/src/evcxr_internal_runtime.rs` | Live typed variable storage; explicit handling of type changes |
| `evcxr/src/runtime.rs` | Persistent execution worker; keep loaded code alive while values/destructors need it |
| `evcxr/src/eval_context.rs` | Separate candidate metadata, compilation, execution, and value packing |
| `evcxr/src/statement_splitter.rs` | Parse actual Rust syntax for statements/items/expressions |
| `evcxr/src/code_block.rs` | Map generated diagnostics back to user code |
| `evcxr_jupyter/src/core.rs` | Request routing, output publication, busy/idle status, control handling |
| `evcxr_jupyter/src/connection.rs` and `jupyter_message.rs` | Signed multipart transport, routing identities, and parent headers |
| `evcxr_jupyter/src/control_file.rs` and `install.rs` | Connection settings and kernelspec installation |
| `evcxr_runtime/src/lib.rs` | MIME bundles and display output |

This milestone records architectural ideas; it imports no Evcxr implementation.
Any later source reuse must preserve the applicable upstream license and notices.
