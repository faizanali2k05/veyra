# Architecture

The dependency direction is deliberately one-way:

```text
source -> lexer -> parser -> AST
                         |-> ordinary types --|
                         |-> policy/flow check-|-> interpreter
                         |                     `-> bytecode compiler -> validator -> VM
planned: policy registry -> capability issuer -> sink adapters -> audit store
stdlib/CLI --------------------------------------------------------------^
```

`veyra-source` owns source spans and diagnostic rendering. `veyra-lexer` turns
UTF-8 source into spanned tokens. `veyra-ast` is representation-only.
`veyra-parser` is recursive descent with Pratt expression parsing.
`veyra-sema` owns name resolution and static typing. The tree-walk
`veyra-interpreter` is the semantic reference. `veyra-bytecode` and `veyra-vm`
implement the independent compiled execution path. `veyra-stdlib`
describe host capabilities explicitly; the CLI composes the pipeline.

No frontend stage delegates to an existing language, and Veyra source will never
be transpiled for execution. Malformed user input is represented as diagnostics,
not panics. Unsafe Rust is forbidden workspace-wide.

## Execution engines

The interpreter evaluates checked ASTs directly. The bytecode compiler lowers the
same AST into constants, lexical operations, arithmetic, jumps, calls, aggregate
operations, and range-loop instructions. Differential tests execute representative
programs through both paths and compare output.

## VM and managed objects

Bytecode is versioned and validated before execution. The VM uses an operand stack
and explicit call frames and rejects malformed bytecode without panics. Arrays and
structs use reference-counted managed storage in safe Rust. Assignment shares
managed aggregates and equality is structural. Cycles are safe but not reclaimed;
the limitation is documented explicitly.

## Policy-aware pipeline

Ordinary types and security labels are checked in distinct, composable passes.
The policy environment assigns canonical policy IDs; expression analysis computes
label joins and provenance; function summaries describe argument/result and effect
dependencies; a program-counter label bounds effects in sensitive control
contexts. Only checked ASTs may be lowered to executable bytecode.

The implemented trusted boundaries are `println` and `write_file`; their static
checks run before lowering. Arbitrary native calls are not supported. The planned
release instruction will accept only an opaque VM capability minted by the trusted
issuer, checks policy, purpose, destination, and transformation constraints, and
writes an audit event before completing. Bytecode validation will reject
fabricated capability constants and calls to undeclared native sinks. None of the
capability, issuer, audit-store, or network/database adapter machinery exists in
the current bytecode format.

The bytecode validator proves structural safety, not information-flow provenance.
Only bytecode produced from source by the trusted checking pipeline is inside the
current policy boundary. Running an untrusted `.vyb` file does not establish that
it passed policy checking.

