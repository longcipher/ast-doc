# ast-doc

[![DeepWiki](https://deepwiki.com/badge.svg)](https://deepwiki.com/longcipher/ast-doc)
[![Context7](https://img.shields.io/badge/Website-context7.com-blue)](https://context7.com/longcipher/ast-doc)
[![crates.io](https://img.shields.io/crates/v/ast-doc.svg)](https://crates.io/crates/ast-doc)
[![docs.rs](https://docs.rs/ast-doc/badge.svg)](https://docs.rs/ast-doc)

![ast-doc](https://socialify.git.ci/longcipher/ast-doc/image?font=Source+Code+Pro&language=1&name=1&owner=1&pattern=Circuit+Board&theme=Auto)

AST-powered code documentation tool for generating optimized `llms.txt` files from codebases.

## Overview

`ast-doc` is a Rust CLI tool that combines broad file traversal with deep AST-based semantic parsing to create optimized documentation. It uses a four-stage pipeline:

1. **Ingestion** — File discovery, git metadata capture, directory tree generation
2. **Parser** — tree-sitter AST extraction with pre-computed strategy variants
3. **Scheduler** — Token budget optimization with intelligent degradation
4. **Renderer** — Markdown assembly with anti-bloat rules

## Supported Languages

- Rust (`.rs`)
- Python (`.py`)
- TypeScript/JavaScript (`.ts`, `.tsx`, `.js`, `.jsx`)
- Go (`.go`)
- C (`.c`, `.h`)

## Installation

### As an Agent Skill

Install this skill for use with AI coding agents:

```bash
npx skills add longcipher/ast-doc
```

### From Source

```bash
cargo install --path bin/ast-doc
```

### From crates.io

```bash
cargo install ast-doc
```

## Features

- **Four-stage pipeline**: Ingestion → AST Parser → Token Scheduler → Renderer
- **Output strategies**: Full, NoTests (strip tests), Summary (signatures only)
- **Token budget management**: Configurable `--max-tokens` with automatic degradation
- **Core file protection**: Mark files with `--core` patterns that never get degraded
- **Git context**: Automatic branch, commit, and diff inclusion (disable with `--no-git`)
- **Directory tree**: Visual project structure with language annotations (disable with `--no-tree`)
- **Glob filtering**: Include/exclude patterns for fine-grained file selection
- **Anti-bloat rules**: Compress blank lines, trim trailing whitespace
- **BDD acceptance tests**: Gherkin scenarios with `cucumber-rs`
- **TDD inner loop**: Unit tests with `cargo test`
- **Property tests**: `proptest` in the standard test flow

## Usage

### Basic Usage

```bash
# Generate llms.txt to stdout
ast-doc .

# Write to a file
ast-doc . --output llms.txt

# Set token budget (default: 128,000)
ast-doc . --max-tokens 64000
```

### Output Strategies

```bash
# Full source code (default)
ast-doc . --strategy full

# Strip test modules and functions
ast-doc . --strategy no-tests

# Signatures only, no implementations
ast-doc . --strategy summary
```

### Core Files Protection

```bash
# Core files always use Full strategy, never degraded
ast-doc . --core "src/main.rs" --core "src/lib.rs" --strategy summary
```

### File Filtering

```bash
# Include only Rust files
ast-doc . --include "*.rs"

# Exclude test files
ast-doc . --exclude "*test*"

# Combine include/exclude
ast-doc . --include "*.rs" --exclude "target/**"
```

### Git and Tree Options

```bash
# Skip git context
ast-doc . --no-git

# Skip directory tree
ast-doc . --no-tree

# Copy to clipboard (not yet implemented)
ast-doc . --copy
```

### Verbose Logging

```bash
ast-doc . --verbose
```

## Quick Start (Development)

```bash
just setup
just check
just test
just bdd
just test-all

# Run the CLI
cargo run -p ast-doc -- --help
cargo run -p ast-doc -- .
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

Apache-2.0
