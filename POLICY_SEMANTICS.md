# Core Policy Semantics

## Policy declarations

```veyra
policy HospitalPrivate {
    owner: "hospital-a";
    export: deny;
}
```

The current compiler supports named confidentiality policies with a stable owner
string and a deny export value. Future syntax may add reader principals and
destination classes, but must preserve deny-by-default behavior.

## Labels and lattice

Let `PolicyId` be the compiler-assigned canonical identity of a policy. A label is
a finite set of policy IDs:

```text
ℓ ::= Public | {p1, ..., pn}
Public = ∅
ℓ1 ⊔ ℓ2 = ℓ1 ∪ ℓ2
ℓ1 ⊑ ℓ2 iff ℓ1 ⊆ ℓ2
```

More policies mean at least as restrictive a value. Joining never discards an
owner's restriction. A public sink has clearance `Public`, so a protected label
does not flow to it because `{P} ⊄ ∅`.

The current compiler representation stores canonical policy names and provenance
spans. The planned scalable representation assigns dense `PolicyId` values and
uses an interned sorted set/bitset plus a provenance DAG. Runtime values do not
carry labels after successful static checking; dynamic adapters carry policy
metadata only where runtime authorization is required.

## Core rules

The following rules omit ordinary type premises for readability.

```text
[PUBLIC]       Γ; Π; pc ⊢ literal : τ @ Public

[PRIVATE]      P ∈ Π     Γ; Π; pc ⊢ e : τ @ ℓ
               --------------------------------
               Γ; Π; pc ⊢ private<P>(e) : τ @ (ℓ ⊔ {P})

[OP]           Γ; Π; pc ⊢ e1 : τ1 @ ℓ1
               Γ; Π; pc ⊢ e2 : τ2 @ ℓ2
               --------------------------------
               Γ; Π; pc ⊢ e1 op e2 : τ @ (ℓ1 ⊔ ℓ2)

[CALL]         label(result) includes the instantiated function summary and
               every argument dependency declared by that summary.

[ASSIGN]       Γ(x) = τ @ ℓx     Γ; Π; pc ⊢ e : τ @ ℓe
               ℓe ⊔ pc ⊑ ℓx
               --------------------------------
               Γ; Π; pc ⊢ x = e ok

[SINK]         Γ; Π; pc ⊢ e : τ @ ℓe     ℓe ⊔ pc ⊑ clearance(destination)
               ------------------------------------------------------------
               Γ; Π; pc ⊢ sink(destination, e) ok
```

Veyra's inferred local labels are flow-sensitive and monotonically raised: an
assignment can add restrictions but ordinary code cannot remove them.

## Implicit flows and `pc`

For a conditional or loop guard labeled `ℓg`, bodies are checked under:

```text
pc' = pc ⊔ ℓg
```

Every checked effect—including output, mutation, return, and selected runtime
error channels—includes `pc'` in its effective label. Therefore a public string
printed only in a protected branch is rejected. Loop analysis raises labels to a
fixed point. Branch updates are monotonic: once a binding becomes protected, the
checker does not lower it again.

Function bodies are analyzed at each call with argument labels and the caller's
`pc`; return labels are joined and passed back to the caller. Recursive cycles use
a conservative join of arguments and `pc`. This is an implementation strategy,
not a persistent policy-polymorphic summary format.

## Capability-controlled release

There is no `declassify(e)`, label cast, `release` built-in, or bytecode downgrade
instruction. The intended future release expression requires an opaque
runtime-created capability:

```text
release value using capability
    for "treatment-summary"
    to Console("clinician-terminal")
```

A future capability is an unforgeable VM object containing at least: capability ID,
issuer, policy IDs it may relax, purpose, destination class and identity,
permitted transformation/schema, expiry, use count/replay state, and audit
correlation ID. Source code cannot construct this type, deserialize it, compare
its representation, or forge its bytecode constant.

Its authorization must require all policy owners being relaxed, exact purpose and
destination compatibility, transformation compatibility, validity/replay checks,
and `pc` no more restrictive than the capability's control context. Both success
and rejection must produce an audit event. Audit failure must be fail-closed.

These capability and audit rules are specification requirements, not implemented
features.

