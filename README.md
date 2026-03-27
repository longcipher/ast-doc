# Rust Workspace Template

Rust workspace template for `bin/` CLI crates and `crates/` shared libraries with Gherkin + `cucumber-rs` for BDD and `cargo test` for TDD.

## Features

- `bin/` + `crates/` workspace layout
- CLI example (`bin/cli-app`) using `clap`
- Shared library example (`crates/common`)
- BDD acceptance tests with Gherkin + `cucumber-rs`
- TDD inner loop with `cargo test`
- Property tests with `proptest` inside the normal `cargo test` flow
- Optional fuzzing with `cargo-fuzz` for parser-like, protocol, or `unsafe`-heavy crates
- Optional benchmarks with Criterion for performance-sensitive crates
- Strict workspace lint configuration
- `just` commands for format/lint/test/bdd/build

## Quick Start

```bash
just setup
just check
just test
just bdd
just test-all

# Run the example CLI
cargo run -p cli-app -- --name Rust
```

## Testing Matrix

- BDD via `features/*.feature` plus `just bdd` remains the acceptance contract.
- Example-based crate-local unit tests remain the default inner loop for named business cases and edge cases.
- `proptest` lives in the ordinary `cargo test` path when the rule is an invariant across many valid inputs.
- Advanced modes are opt-in: use `cargo-fuzz` only for hostile-input or `unsafe`-heavy crates, and add Criterion only when the work has a real performance target.

## BDD + TDD Workflow

1. Write a failing Gherkin scenario in `features/*.feature`.
2. Write a failing crate-local unit or property test in the affected crate to drive the inner loop.
3. Implement the smallest shared Rust API needed to satisfy the test.
4. Run `just test` to exercise deterministic unit tests and any `proptest` properties together.
5. Re-run `just bdd` to confirm the acceptance scenario passes.

Use example-based unit tests for named business cases and edge cases that should stay readable. Use `proptest` when the rule is an invariant, such as totals matching line-item arithmetic or checkout always emptying the cart.

## Project Convention

- Put executable crates under `bin/*`
- Put reusable library crates under `crates/*`
- Keep shared dependencies in root `[workspace.dependencies]`

## Common Commands

```bash
just format
just lint
just test
just bdd
just test-all
just build
```

`just test` runs the usual `cargo test --all-features` flow, so colocated `proptest` coverage in crate test modules stays in the standard inner loop rather than a separate test layer.

## Conditional Benchmark Guidance

Do not add Criterion or a benchmark scaffold to every new workspace by default. Most business logic and CRUD-style crate work should stay on the ordinary `just test` plus `just bdd` path unless the planned feature has an explicit latency SLA, throughput target, or known hot path worth measuring.

When performance-sensitive code appears, add Criterion only in the affected crate and benchmark the hot path that carries the requirement. That keeps the default template lean while still using the standard Rust benchmark tool when the work genuinely needs measurement.

## Conditional Fuzzing Guidance

Do not add `cargo-fuzz` targets to every new workspace by default. The standard Rust template is enough for ordinary business logic, CRUD-style services, and shared domain crates that only handle trusted or well-formed inputs.

Reach for fuzzing when a specific crate starts handling hostile input or high-risk memory behavior, especially when it:

- parses free-form text or file formats,
- implements protocol framing or message decoding,
- decodes binary formats or other untrusted payloads,
- or relies on substantial `unsafe` code.

When one of those conditions applies, enable fuzzing only in the affected crate and use the normal Cargo workflow rather than baking a `fuzz/` directory into every starter:

```bash
cd crates/<crate-name>
cargo fuzz init
cargo fuzz run <target-name>
```

That keeps the default template lean while still pointing parser-like, protocol, binary-decoding, or `unsafe`-heavy crates to the standard `cargo-fuzz` layout when they actually need it.

## License

MIT
