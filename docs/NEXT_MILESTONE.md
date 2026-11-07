# Next Implementable Milestone: Capability Release and Sink Contracts

## Objective

Add the first authorized path for deliberately releasing protected data without
adding a generic downgrade primitive. The same milestone should define one typed
sink contract shared by console, files, and future network/database adapters.

## Required design

1. Introduce an opaque runtime capability that source literals, deserialization,
   ordinary constructors, and bytecode constants cannot create.
2. Bind each capability to policy IDs, issuer, purpose, destination class and
   identity, permitted transformation/schema, expiry, use count, and audit ID.
3. Check the value label and program-counter label against every owner whose
   restriction would be relaxed.
4. Validate capabilities in the trusted runtime after bytecode validation and
   immediately before the sink effect.
5. Write success and rejection audit events. If durable audit recording fails,
   reject the release.
6. Generalize sink clearance checking behind typed adapters; add log,
   serialization, and HTTP only after their contracts and tests exist.

## Required adversarial tests

- constructing, copying, serializing, or embedding a forged capability;
- wrong purpose, destination, owner, transformation, expiry, or use count;
- replay and cross-module laundering;
- release under a protected `pc` not authorized by the capability;
- malformed bytecode attempting to invoke the release instruction directly;
- audit-store failure and concurrent replay races;
- successful release limited to the exact authorized sink and transformation.

## Definition of done

- there is still no `declassify`, label cast, or unrestricted `release(value)`;
- the VM rejects fabricated capability representations before execution;
- every accepted and rejected attempt has a testable structured audit record;
- interpreter and VM agree on authorization outcomes;
- threat model, TCB, bytecode specification, CLI diagnostics, and examples are
  updated;
- timing, termination, resource, residency, cryptographic, and downstream-copy
  limitations remain explicit.
