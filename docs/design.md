# ast-doc: AST-Powered Code Documentation Engine

## 🎯 Product Overview

**ast-doc** is a pure local CLI tool that combines the broad file traversal capabilities of `code2prompt` with the deep AST-based semantic parsing of `codebank` to generate optimized `llms.txt` documentation from codebases.

### Core Value Proposition

- **Zero Hallucination**: Pure local AST parsing, no external APIs or AI inference
- **Token Optimization**: 60%-80% token reduction while preserving architectural skeleton
- **Privacy First**: No data leaves your machine - pure local binary execution
- **LLM-Ready Output**: Standard Markdown format optimized for LLM consumption

---

## 🏗️ System Architecture

### Four-Stage Pipeline

#### Phase 1: Smart Ingestion (from code2prompt)

- **Intelligent Filtering**: Deep parsing of `.gitignore` and custom `.astdocignore` files
- **Metadata Capture**:
  - Project directory tree structure
  - Current Git branch and status
  - Uncommitted changes (Git diff)
  - Recent commit history
- **File Discovery**: Recursive traversal with glob pattern support
- **Local Tokenizer**: Fast Rust-based token counting (tiktoken-compatible)

#### Phase 2: AST Semantic Extraction (from codebank)

For each discovered source file, invoke local `tree-sitter` engine:

- **Multi-Language Support**:
  - Rust (full support)
  - Python (functions, classes, modules)
  - TypeScript/JavaScript (functions, classes, interfaces, exports)
  - Go (packages, functions, structs, interfaces, methods)
  - C (basic support with planned enhancements)

- **Three Output Strategies**:
  1. **Full Mode**: Complete source code preservation
  2. **NoTests Mode**: Precise test module detection and removal
     - Rust: `#[cfg(test)]` modules
     - Python: `def test_*` functions, `class Test*` classes
     - TypeScript: `describe()`, `it()`, `test()` blocks
     - Go: `func Test*` functions
  3. **Summary Mode**: Public interface extraction only
     - Class/Struct/Interface signatures
     - Function signatures with docstrings
     - Trait implementations
     - Remove function bodies with `// Implementation omitted`

#### Phase 3: Dynamic Token Scheduler

Local degradation algorithm for token budget management:

```text
Input: --max-tokens 50000

1. Calculate base overhead (directory tree, git state)
2. Priority allocation:
   - User-specified --core files → locked in Full Mode
   - All others start in Full Mode
3. Adaptive degradation loop:
   IF total_tokens > 50000:
     a. Degrade test-heavy files → NoTests Mode
     b. IF still exceeding:
        Sort by file size (descending) or non-core directories
        Degrade to Summary Mode until budget met
```

#### Phase 4: llms.txt Rendering

- **Anti-Bloat Rules**: Compress consecutive blank lines, remove excess whitespace
- **Standard Markdown**: Clean hierarchical structure for optimal LLM parsing
- **Strategy Annotations**: Each file marked with its processing mode and token savings

---

## 📝 Output Format: llms.txt Specification

```text
# Repository: {Project_Name}

> {Project_Description_or_System_Prompt}
> ℹ️ Note: This codebase has been optimized using AST trimming to fit token limits.
> Files are presented in Full, NoTests, or Summary (signatures only) modes.

## 🌳 Structure & Symbol Index

### Directory Tree

```text
src/
├── core/
│   ├── engine.rs      [Full]
│   └── parser.rs      [NoTests]
├── utils/
│   └── math.rs        [Summary]
└── Cargo.toml         [Full]
```

### 🔄 Git Context

- **Branch**: `feature/ast-parser`
- **Latest Commit**: `feat: implement tree-sitter based summary mode`
- **Uncommitted Changes**: Modified `src/core/engine.rs`

---

## 📄 Source Files

### 📁 File: `src/core/engine.rs`

*Strategy: Full | Tokens: 345*

```rust
pub struct Engine {
    pub version: String,
}

impl Engine {
    pub fn start(&self) {
        println!("Engine {} starting...", self.version);
        self.initialize_modules();
    }

    fn initialize_modules(&self) {
        // Init logic here
    }
}
```

### 📁 File: `src/core/parser.rs`

*Strategy: NoTests | Tokens: 120 (Saved: 450)*

```rust
pub fn parse_input(input: &str) -> Result<ASTNode, ParseError> {
    let tokens = tokenize(input);
    build_tree(tokens)
}
// ✂️ test module omitted
```

### 📁 File: `src/utils/math.rs`

*Strategy: Summary | Tokens: 45 (Saved: 800)*

```rust
/// Calculates the complex mathematical sequence
pub fn calculate_sequence(input: Vec<i32>) -> i32;

/// Advanced mathematical traits
pub trait AdvancedMath {
    fn compute_derivative(&self) -> f64;
}
// ✂️ implementations omitted
```

```text

---

## 💻 CLI Interface

### Basic Usage

```bash
# Generate optimized llms.txt for current directory
ast-doc .

# Copy to clipboard
ast-doc . --copy

# Strict token limit with core files protected
ast-doc . --max-tokens 30000 --core "src/core/**" --output ./my-project.txt

# Specify output strategy for all files
ast-doc . --strategy summary --output summary.txt
```

### Advanced Options

```bash
ast-doc [OPTIONS] <PATH>

Options:
  -o, --output <FILE>         Output file path (default: stdout)
  -m, --max-tokens <NUM>      Maximum token budget (default: 100000)
  -c, --core <PATTERN>        Glob pattern for core files (never degrade)
  -s, --strategy <STRATEGY>   Default strategy: full|no-tests|summary
      --include <PATTERN>     Include file patterns
      --exclude <PATTERN>     Exclude file patterns
      --no-git                Skip git context
      --no-tree               Skip directory tree
      --copy                  Copy output to clipboard
  -v, --verbose               Verbose output
  -h, --help                  Print help
```

---

## 📊 Terminal Output Report

```text
$ ast-doc . --max-tokens 30000 --output llms.txt

🚀 Analyzing Codebase [my_project]...
🌳 Parsing directory tree and git state...
⚙️ Running AST semantic engine (Tree-sitter)...

✅ Generation Complete! Saved to `llms.txt`.

📊 Optimization Report (Target: < 30,000 Tokens)
==================================================
Total Files Processed : 142 files
Raw Project Tokens    : ~124,500 🔴 (Would exceed budget!)
Final Prompt Tokens   :   28,450 🟢 (Safe for LLM Context)

Strategy Breakdown:
- [Full]     : 12 files  (18,000 tokens) -> 100% code
- [NoTests]  : 35 files  ( 5,400 tokens) -> ✂️ Saved 15,000 tokens
- [Summary]  : 95 files  ( 4,000 tokens) -> ✂️ Saved 80,000+ tokens
- [Metadata] : Tree & Git( 1,050 tokens)

💡 Magic: Your context is now 75% smaller but retains 100% of the architectural skeleton!
==================================================
```

---

## 🔧 Technical Implementation

### Core Dependencies

```toml
[dependencies]
# From code2prompt patterns
ignore = "0.4"           # .gitignore parsing
git2 = "0.19"            # Git integration
glob = "0.3"             # Pattern matching
tiktoken-rs = "0.5"      # Token counting

# From codebank patterns
tree-sitter = "0.22"     # AST parsing core
tree-sitter-rust = "0.21"
tree-sitter-python = "0.21"
tree-sitter-typescript = "0.21"
tree-sitter-go = "0.21"
tree-sitter-c = "0.21"

# Common
clap = { version = "4.5", features = ["derive"] }
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
eyre = "0.6"
tracing = "0.1"
```

### Module Structure

```text
src/
├── main.rs              # CLI entry point
├── lib.rs               # Public API
├── config.rs            # Configuration handling
├── error.rs             # Error types
├── ingestion/           # Phase 1: File discovery
│   ├── mod.rs
│   ├── walker.rs        # Directory traversal
│   ├── filter.rs        # .gitignore/.astdocignore
│   └── git.rs           # Git metadata
├── parser/              # Phase 2: AST parsing
│   ├── mod.rs
│   ├── lang/            # Language-specific parsers
│   │   ├── rust.rs
│   │   ├── python.rs
│   │   ├── typescript.rs
│   │   ├── go.rs
│   │   └── c.rs
│   └── strategies.rs    # Full/NoTests/Summary
├── scheduler/           # Phase 3: Token optimization
│   ├── mod.rs
│   └── optimizer.rs     # Degradation algorithm
└── renderer/            # Phase 4: Output generation
    ├── mod.rs
    └── llms_txt.rs      # Markdown rendering
```

---

## 🎯 Why This Design is Optimal

1. **Simplicity**: Single binary, no backend, no MCP complexity
2. **LLM Alignment**: Follows `llms.txt` standard, pure Markdown output
3. **Solves Real Pain**: Most tools either dump everything (token explosion) or use RAG (loses global structure)
4. **AST + Scheduler**: Perfect balance of "global view (skeleton)" and "local detail (flesh)"
5. **Blazing Fast**: Rust implementation with tree-sitter for millisecond parsing

---

## 🚀 Future Enhancements

- [ ] Streaming output for very large codebases
- [ ] Caching support for incremental updates
- [ ] Custom output templates (Handlebars)
- [ ] Plugin system for custom parsers
- [ ] VS Code extension integration
- [ ] GitHub Action for CI/CD pipelines
