# ast-doc

[![DeepWiki](https://deepwiki.com/badge.svg)](https://deepwiki.com/longcipher/ast-doc)
[![crates.io](https://img.shields.io/crates/v/ast-doc.svg)](https://crates.io/crates/ast-doc)
[![docs.rs](https://docs.rs/ast-doc/badge.svg)](https://docs.rs/ast-doc)

AST-powered code documentation tool for generating optimized `llms.txt` files from codebases.

## Overview

`ast-doc` is a Rust CLI that combines broad file traversal with deep AST-based semantic parsing to create optimized documentation. It uses a four-stage pipeline:

1. **Ingestion** — File discovery, git metadata capture, directory tree generation
2. **Parser** — tree-sitter AST extraction with pre-computed strategy variants
3. **Scheduler** — Token budget optimization with intelligent degradation
4. **Renderer** — Markdown assembly with anti-bloat rules

## Supported Languages

### Core (deep analysis)

| Language | Extensions |
|----------|------------|
| Rust | `.rs` |
| Python | `.py` |
| TypeScript/JavaScript | `.ts`, `.tsx`, `.js`, `.jsx` |
| Go | `.go` |
| C | `.c`, `.h` |

### Extended

With the `lang-pack` feature, 50+ additional languages are supported via `tree-sitter-language-pack` (Java, Ruby, Kotlin, Swift, etc.).

## Installation

### As an Agent Skill

Install this skill for use with AI coding agents:

```bash
npx skills add longcipher/ast-doc
```

### From crates.io

```bash
cargo install ast-doc
```

### From source

```bash
cargo install --path bin/ast-doc
```

## Usage

```bash
# Generate llms.txt to stdout
ast-doc .

# Write to a file
ast-doc . --output llms.txt

# Set token budget (default: 128,000)
ast-doc . --max-tokens 64000

# Use summary mode (signatures only)
ast-doc . --strategy summary

# Strip tests
ast-doc . --strategy no-tests

# Protect core files from degradation
ast-doc . --core "src/main.rs" --core "src/lib.rs" --strategy summary

# Filter files
ast-doc . --include "*.rs" --exclude "target/**"

# Skip git context and directory tree
ast-doc . --no-git --no-tree
```

## Development

```bash
# Install tools
just setup

# Run full CI
just ci

# Individual steps
just lint
just test
just bdd
just build
```

## Feature Flags

| Feature | Description | Default |
|---------|-------------|---------|
| `lang-rust` | Rust parser | ✓ |
| `lang-pack` | 50+ languages via tree-sitter-language-pack | ✓ |
| `lang-python` | Python parser | ✗ |
| `lang-typescript` | TypeScript/JavaScript parser | ✗ |
| `lang-go` | Go parser | ✗ |
| `lang-c` | C parser | ✗ |
| `all-languages` | Enable all language parsers | ✗ |
| `hotpath` | Profiling instrumentation | ✗ |

## Testing

- **BDD**: Gherkin scenarios in `features/*.feature`, run with `just bdd`
- **Unit tests**: Colocated `#[cfg(test)]` modules, run with `just test`
- **Property tests**: `proptest` in standard `cargo test` flow

## Project Structure

```text
bin/          CLI binary crates
crates/       Reusable library crates
features/     BDD Gherkin scenarios
```

## License

Apache-2.0
