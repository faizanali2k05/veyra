# Contributing

Keep changes milestone-sized and update the specification whenever observable
semantics change. Add both success and failure tests. Public frontend/runtime APIs
must return structured errors for bad Veyra input and must not use `unwrap`,
`expect`, or unchecked indexing on user-controlled data.

Before submitting a change, run:

```sh
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

Use safe Rust. If a future change truly needs `unsafe`, it requires a written
safety argument and a workspace policy change reviewed independently. Keep crate
dependencies acyclic and pointed in the direction documented in
`ARCHITECTURE.md`. Commit messages should state the affected milestone.

