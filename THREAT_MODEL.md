# Veyra Threat Model

## Assets

- values protected by declared confidentiality policies;
- release capabilities and their signing/issuance keys;
- policy definitions, principal identities, and authority relationships;
- audit records describing allowed and rejected release attempts;
- integrity of compiled bytecode and trusted sink configuration.

## Adversary

Assume an attacker can author arbitrary Veyra source, choose public inputs,
compose modules, trigger error paths, inspect public outputs, submit malformed
bytecode, and repeatedly invoke a program. The attacker may try direct copying,
derived expressions, implicit branch/loop leaks, exception/error leaks, label
laundering through aggregates or functions, capability fabrication/replay, and
unmediated I/O.

The attacker does not initially control the Veyra compiler/runtime TCB, policy
registry, capability issuer, audit store, OS, or hardware. Compromise of any of
those components is a separate threat requiring sandboxing, signing, operational
controls, or hardware/OS security.

## Implemented controls

- static propagation of confidentiality dependencies;
- program-counter labels for effectful implicit flows;
- sink clearance checks for console and file output;
- function/return and sibling-module propagation;
- conservative checks for division/remainder and bounds-check error channels;
- structural bytecode validation and no label-downgrade instruction;
- diagnostics carrying provenance from label origin to forbidden destination.

HTTP, database, log, serialization, and external-adapter contracts are planned,
not implemented. Unforgeable scoped release capabilities and append-only audit
events are also planned. Until they exist, all label release is unavailable.

Untrusted `.vyb` input is checked for structural validity but carries no proof of
source-level policy checking. It is outside the information-flow guarantee; only
bytecode emitted by the trusted compiler from successfully checked source is in
scope.

## Residual risks

Veyra does not prevent timing, termination, scheduling, cache, power, allocation,
resource-consumption, output-size, or traffic-analysis channels. Conservative
static rules may reject secure programs. Availability and denial of service are
not confidentiality guarantees. File/HTTP/database destinations can copy data
after an authorized release. Policy labels are not physical residency controls.

Concurrency is excluded from the initial security theorem because scheduling and
termination observations require a stronger model. Unsafe/native FFI is outside
the boundary unless it is mediated by a reviewed adapter.

