# ast-doc-core

[![crates.io](https://img.shields.io/crates/v/ast-doc-core.svg)](https://crates.io/crates/ast-doc-core)
[![docs.rs](https://docs.rs/ast-doc-core/badge.svg)](https://docs.rs/ast-doc-core)

Core library for [ast-doc](https://crates.io/crates/ast-doc): a four-stage pipeline for generating optimized `llms.txt` documentation from codebases.

## Pipeline

1. **Ingestion** — File discovery, git metadata capture, directory tree generation
2. **Parser** — tree-sitter AST extraction with pre-computed strategy variants
3. **Scheduler** — Token budget optimization with intelligent degradation
4. **Renderer** — Markdown assembly with anti-bloat rules

## Usage

```rust
use ast_doc_core::{AstDocConfig, OutputStrategy, run_pipeline};
use std::path::PathBuf;

let config = AstDocConfig {
    path: PathBuf::from("."),
    output: None,
    max_tokens: 128_000,
    core_patterns: vec![],
    default_strategy: OutputStrategy::Full,
    include_patterns: vec![],
    exclude_patterns: vec![],
    no_git: false,
    no_tree: false,
    copy: false,
    verbose: false,
};

let result = run_pipeline(&config).expect("pipeline failed");
println!("{}", result.output);
```

## Feature Flags

| Feature | Language | Default |
|---------|----------|---------|
| `lang-rust` | Rust (`.rs`) | Yes |
| `lang-python` | Python (`.py`) | No |
| `lang-typescript` | TypeScript/JavaScript (`.ts`, `.tsx`, `.js`, `.jsx`) | No |
| `lang-go` | Go (`.go`) | No |
| `lang-c` | C (`.c`, `.h`) | No |
| `all-languages` | All of the above | No |

## License

Apache-2.0
