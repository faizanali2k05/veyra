# Roadmap and definition of done

Every milestone requires formatted code, warning-free Clippy, passing workspace
tests, positive and negative fixtures, updated specification text, and an honest
feature/limitation report. A milestone is not done while supported user input can
panic a public pipeline API.

- **M0 — workspace/specification:** Cargo architecture, language grammar,
  contributor docs, CI, fixtures, and the checked arithmetic vertical slice.
- **M1 — lexer/diagnostics:** complete language token set, comments, literals,
  UTF-8 behavior, snapshots, and invalid-input coverage.
- **M2 — parser/AST:** declarations, blocks, functions, control flow, recovery,
  and diagnostic snapshots.
- **M3 — reference interpreter:** environments, functions, recursion, branches,
  loops, and scope tests.
- **M4 — semantic analysis:** resolution, mutability, inference, calls, complete
  returns, and friendly static diagnostics.
- **M5 — bytecode/VM:** documented instructions, compiler, validation,
  disassembly, call frames, and differential interpreter/VM tests.
- **M6 — data/modules:** arrays, strings, structs, indexing, module loading, and a
  documented managed-heap approach.
- **M7 — standard library/tooling:** capabilities plus `run`, `check`, `build`,
  `repl`, `fmt`, and `disasm` as applicable; version 1 builds `.vyb` bytecode.
- **M8 — hardening/release:** fuzzing, malformed-bytecode tests, benchmarks,
  regression and conformance suites, format versioning, packaging, and an honest
  1.0 report.

## Current status

The Veyra 1.0 implementation covers the M0–M8 functional path: full tokens and
AST parsing, reference interpretation, static analysis, versioned bytecode and a
validated VM, data types/modules, standard I/O, CLI tooling, differential engine
tests, malformed-bytecode regressions, examples, and release documentation.
Deliberate product boundaries are listed in `docs/LIMITATIONS.md`.

## Policy-aware security roadmap

- **S0 — research/security specification (complete):** policy lattice, security
  judgments, TCB, threat model, research context, and side-channel limitations.
- **S1 — basic confidentiality labels (complete):** `policy`,
  `private<Policy>(value)`, expression propagation, provenance diagnostics, and
  rejection of direct/derived protected console output.
- **S2 — control/effect labels (complete for the sequential core):**
  program-counter joins for `if`, `while`, and `for`; monotonic loop fixed points;
  return/mutation effects; conservative division, remainder, and bounds-error
  checks; implicit-flow fixtures.
- **S3 — function/module propagation (partial):** call-site argument/result/`pc`
  propagation and cross-module tests are complete. Persistent policy-polymorphic
  public summaries and a more precise recursive fixed point remain open.
- **S4 — uniform trusted sinks (partial):** console and file writes are checked.
  Typed log, serialization, HTTP, database, and external-adapter contracts remain
  open; ambient native I/O is not supported by the language.
- **S5 — capability release:** opaque VM capabilities scoped by owners, purpose,
  destination, transform, expiry, and replay; fail-closed audit events; no generic
  downgrade primitive.
- **S6 — assurance:** malicious bytecode/capability tests, property tests, fuzzing,
  formal core metatheory, differential checker tests, multi-organization examples,
  and an independent TCB review.

The next milestone is S5 together with the missing S4 adapters; see
`docs/NEXT_MILESTONE.md`. It must not introduce a generic label-removal escape
hatch.

