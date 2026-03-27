# ast-doc

[![crates.io](https://img.shields.io/crates/v/ast-doc.svg)](https://crates.io/crates/ast-doc)
[![docs.rs](https://docs.rs/ast-doc/badge.svg)](https://docs.rs/ast-doc)

CLI for generating optimized `llms.txt` documentation from codebases using AST-based semantic parsing.

## Installation

```bash
cargo install ast-doc
```

## Quick Start

```bash
# Generate llms.txt to stdout
ast-doc .

# Write to a file
ast-doc . --output llms.txt

# Set token budget (default: 128,000)
ast-doc . --max-tokens 64000
```

## Output Strategies

```bash
ast-doc . --strategy full        # Full source code (default)
ast-doc . --strategy no-tests    # Strip test modules
ast-doc . --strategy summary     # Signatures only
```

## Supported Languages

- Rust, Python, TypeScript/JavaScript, Go, C

For the library API, see [ast-doc-core](https://crates.io/crates/ast-doc-core).

## License

Apache-2.0
