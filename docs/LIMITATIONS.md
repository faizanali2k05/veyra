# Veyra 1.0 limitations

Veyra 1.0 intentionally omits generics, inheritance, macros, closures,
async/await, packages, native-code compilation, and string indexing. The REPL
evaluates integer expressions rather than retaining a full program environment.
`fmt` validates source and normalizes trailing whitespace; it is not a canonical
pretty-printer.

Module names are single identifiers and resolve to sibling `.vy` files. Module
globals are rejected. Struct types cannot be qualified directly in source;
modules expose behavior through public functions.

Arrays and structs use reference-counted storage. Cyclic object graphs are safe
but are not reclaimed in 1.0. This can leak memory but cannot violate Rust memory
safety. A tracing collector is reserved for a later runtime release.

For static errors originating in an imported module, the CLI currently renders
the span against the entry source. Parsing/loading errors identify the module.

The policy-aware checker covers explicit expression dependencies, conservative
program-counter labels, call/return propagation, sibling modules, console/file
output, and selected error paths. It is flow-sensitive but monotonic, so it may
reject programs whose branches are semantically equivalent. Its call-site
analysis is not a separately checked, policy-polymorphic public summary system.

Capability-authorized release, audit records, HTTP/database/log/serialization
sinks, arbitrary native adapters, concurrency, and formal noninterference proofs
are not implemented. Input and file reads are public unless a program explicitly
wraps them with `private<Policy>`. Timing, termination, allocation, output size,
file metadata, traffic shape, scheduling, and resource usage remain observable
side channels. Veyra must not be described as providing complete privacy or
general noninterference.

