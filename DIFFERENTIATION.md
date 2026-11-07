# Research Context and Practical Differentiation

Veyra does not claim to invent information-flow types or decentralized labels.
Its design builds on a substantial body of work.

## Jif and decentralized labels

Jif extends Java with static and runtime information-flow controls. The
decentralized label model supports multiple principals, ownership, reader
policies, authority, and controlled declassification. These are foundational
ideas for Veyra's policy ownership and lattice. See Myers and Liskov's
[decentralized-label work](https://www.cs.cornell.edu/andru/papers/sp98/paper.html)
and the [Jif DLM documentation](https://www.cs.cornell.edu/jif/doc/jif-3.3.0/dlm.html).

Jif is intentionally expressive, but its Java extension, label syntax, authority
model, runtime label features, and declassification surface create a steep
adoption and audit burden. Veyra's proposed improvement is narrower: immutable-by-
default language semantics, named application policies, destination-aware sinks,
purpose-scoped capability release, provenance-first diagnostics, and an auditable
VM boundary. This is an engineering tradeoff, not a stronger theorem by default.

## Flow Caml

Flow Caml demonstrated constraint-based security type inference for an ML-family
language and handled features such as references and exceptions. Its inference
work strongly informs Veyra's goal of keeping local labels mostly inferred. See
Pottier and Simonet's [Information Flow Inference for
ML](https://www.normalesup.org/~simonet/publis/fpottier-simonet-popl02-long.pdf)
and Simonet's [Flow Caml overview](https://pauillac.inria.fr/~simonet/publis/simonet-flowcaml-nutshell.pdf).

Flow Caml was a research extension translated to OCaml and did not center a
modern operational model of purpose/destination-scoped releases, sink adapters,
and audit records. Veyra can explore that product boundary while retaining a
smaller, explicitly specified type core.

## Program-counter labels and release

Program-counter labels are established machinery for implicit flows; Veyra will
use them rather than inventing a branch heuristic. Hirsch and Cecchetti explain
their relation to effects in [Giving Semantics to Program-Counter Labels via
Secure Effects](https://www.cs.cornell.edu/~ethan/papers/niho.pdf).

Declassification can itself become an attacker-controlled channel. Veyra's
capability plan is motivated by work on [robust
declassification](https://www.cs.cornell.edu/zdance/csfw.pdf): authority alone is
not enough; what, where, why, and under whose influence a release occurs matter.
Capability systems contribute unforgeable, delegable authority, but capabilities
alone do not track data dependencies. Veyra combines the two mechanisms rather
than treating either as sufficient.

## Practical hypothesis

Veyra's testable hypothesis is that an integrated language/VM can make IFC more
usable through named policies, inferred propagation, uniform sink contracts,
actionable provenance, capability-scoped releases, and mandatory audit events.
Usability and assurance must be measured; they are not established by this design
document.

