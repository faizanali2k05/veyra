# Benchmarks

The Criterion frontend benchmark lexes and parses the representative showcase:

```sh
cargo bench -p veyra-parser --bench frontend
```

Benchmark results are machine-dependent and are not committed as performance
claims. The benchmark exists to detect regressions across local revisions.

