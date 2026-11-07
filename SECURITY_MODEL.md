# Veyra Security Model

Status: implemented S1/S2 core with conservative call-site propagation and
console/file sink enforcement.

Veyra's security goal is to make policy-relevant dependencies visible to the
compiler and to reject flows into destinations whose clearance is too weak. This
is language-based information-flow control, not a claim of complete privacy.

## Security judgment

Ordinary typing and policy typing are separate judgments:

```text
Γ; Π; pc ⊢ e : τ @ ℓ
Γ; Π; pc ⊢ s ok
```

`Γ` maps names to ordinary types and labels. `Π` is the policy environment. `pc`
is the program-counter label. An expression has an ordinary type `τ` and a
confidentiality label `ℓ`. Keeping the judgments separate prevents policy data
from being confused with runtime representation.

The compiler implements expression labels and a program-counter (`pc`) label.
Branches and loop bodies join their guard labels into `pc`; assignment, return,
output, and selected runtime-error channels include that control dependency.
Function bodies are reanalyzed at call sites with their argument and caller `pc`
labels. This is deliberately conservative and is not a proof of general
noninterference.

## Security boundary

Inside the enforced boundary:

- Veyra source accepted by the type and policy checkers;
- bytecode emitted by that checked compiler and validated by the VM;
- interpreter and VM operations defined by the language;
- the implemented console and file sink checks.

The trusted computing base (TCB) is the Rust frontend, semantic/policy checker,
bytecode compiler and validator, VM/interpreter, and implemented sink adapters.
A future capability issuer, policy registry, and audit store would enlarge that
TCB and require separate review.

Outside the boundary are the operating system, hardware, administrators, native
libraries, arbitrary FFI, compromised sink services, and observations through
time, termination, allocation, CPU use, file size, traffic shape, or other side
channels. Native calls will be forbidden unless wrapped by a declared trusted
adapter with an information-flow contract.

## Guarantees and non-guarantees

The implemented property is a conservative source-level rejection rule for
protected dependencies reaching `println`, `write_file`, division/remainder
failure, or bounds-check failure. It covers explicit expression dependencies,
`if`/loop program-counter dependencies, function calls/returns, and sibling
modules. Labels are checked before lowering through the supported CLI pipeline.

This is not a formal proof of termination-insensitive noninterference. In
particular, it does not mediate future HTTP/database/log/serialization APIs,
untrusted bytecode producers, or native FFI. No authorized release mechanism is
implemented: attempts to call `release` are ordinary unresolved-call errors.

Labels do not encrypt data, guarantee physical residency, prove a remote service
honest, or make federated learning/model updates private. Cryptography,
differential privacy, secure multiparty computation, and trusted hardware are
separate mechanisms requiring separate designs and validation.

