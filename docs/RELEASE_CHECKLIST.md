# Release Checklist

Veyra is ready for a tagged release only when every applicable item is checked
against the candidate commit. A green checklist does not expand the security
guarantees in `SECURITY_MODEL.md`.

## Code and compatibility

- [ ] `cargo fmt --check` succeeds.
- [ ] strict workspace Clippy succeeds with warnings denied.
- [ ] all workspace tests succeed on every supported CI platform.
- [ ] release build succeeds with the documented minimum stable Rust version.
- [ ] `.vyb` magic/version compatibility and malformed-bytecode rejection pass.
- [ ] no unsafe Rust is introduced without an explicit architecture and threat
      model review.

## Language demonstrations

- [ ] the multi-file example checks and prints `49` through the VM.
- [ ] the same representative programs agree in the interpreter and VM.
- [ ] the static type-error fixture is rejected with a source diagnostic.
- [ ] direct, derived, implicit, function, module, file, and error-path policy
      fixtures are rejected.
- [ ] the protected-computation positive fixture is accepted.
- [ ] attempted unauthorized release is rejected.

## Security and documentation

- [ ] the feature matrix matches executable behavior.
- [ ] the security model, threat model, TCB, policy rules, limitations, and
      research context match the implementation.
- [ ] timing, termination, resource, metadata, residency, FFI, and downstream
      disclosure limitations remain explicit.
- [ ] new sinks have typed contracts and negative policy tests.
- [ ] any release capability has forgery, replay, scope, expiry, destination,
      purpose, transformation, and fail-closed audit tests.
- [ ] no documentation claims that labels provide encryption, physical data
      residency, differential privacy, MPC, or inherently private model updates.

## Distribution

- [ ] README installation commands work from a clean checkout.
- [ ] license and package metadata are reviewed before public publication.
- [ ] original SVG assets render at icon and documentation sizes.
- [ ] changelog and completion report name only verified changes.
- [ ] the release artifact and checksum are generated from the tagged commit.
