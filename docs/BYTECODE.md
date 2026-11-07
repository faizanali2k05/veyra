# Bytecode format and VM

Veyra bytecode files begin with ASCII `VYBC`, followed by a serialized bytecode
program whose internal version is 1. The loader rejects missing magic, malformed
serialization, unsupported versions, and out-of-range jump targets.

The instruction set includes constants, lexical loads/definitions/stores,
checked arithmetic and comparisons, scopes, jumps, calls and returns, arrays,
structs, indexing, range iteration, and module lookup. The VM uses an operand
stack and explicit call frames.

Runtime arrays and structs are reference-counted, memory-safe managed objects.
No unsafe Rust is used. Values compare structurally.

