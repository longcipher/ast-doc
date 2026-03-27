# Design: ast-doc Pipeline

| Metadata | Details |
| :--- | :--- |
| **Status** | Revised |
| **Created** | 2026-03-26 |
| **Revised** | 2026-03-26 |
| **Scope** | Full |

## Executive Summary

ast-doc is a pure local CLI tool that combines file traversal capabilities (inspired by code2prompt) with deep AST-based semantic parsing (inspired by codebank) to generate optimized `llms.txt` documentation from codebases. It applies a four-stage pipeline — Ingestion, AST Extraction, Token Scheduling, and Rendering — to achieve 60-80% token reduction while preserving architectural skeleton for LLM consumption.

## Source Inputs & Normalization

**Source material:** `docs/design.md` (314 lines) — a complete product design document covering system architecture, CLI interface, output format specification, module structure, and technical implementation details.

**Normalization applied:**

- Extracted 10 discrete requirements (R1-R10) from the design document
- Identified the template-derived project identity issue: current repo uses `cli-app` (hello world) and `common` (shopping cart example) as placeholder names
- Mapped the four-stage pipeline to concrete module boundaries
- Identified reusable patterns from vendored reference repos (`code2prompt/` for ingestion patterns, `codebank/` for AST parsing patterns)

**Ambiguities resolved as assumptions:**

- **A1**: The `code2prompt/` and `codebank/` directories are reference implementations to draw patterns from, not libraries to depend on directly. ast-doc will reimplement needed functionality using its own modules.
- **A2**: The existing `features/checkout.feature` and `common` crate are template artifacts that will be replaced entirely.
- **A3**: The `.astdocignore` file format follows the same syntax as `.gitignore` (handled by the `ignore` crate).
- **A4**: "C (basic support)" means function and struct extraction only, with planned enhancements deferred.

## Requirements & Goals

### Functional Requirements

| ID | Requirement |
|----|-------------|
| R1 | CLI tool that generates optimized `llms.txt` documentation from codebases |
| R2 | Four-stage pipeline: Ingestion -> AST Extraction -> Token Scheduling -> Rendering |
| R3 | Smart Ingestion: .gitignore + .astdocignore parsing, git metadata (branch, diff, commits), file discovery with glob, local tokenizer |
| R4 | AST Semantic Extraction via tree-sitter for Rust, Python, TypeScript/JavaScript, Go, C with Full/NoTests/Summary strategies |
| R5 | Dynamic Token Scheduler with adaptive degradation (Full -> NoTests -> Summary) and core file protection |
| R6 | llms.txt rendering with anti-bloat rules, standard markdown, strategy annotations |
| R7 | CLI interface: path, output, max-tokens, core, strategy, include/exclude, no-git, no-tree, copy, verbose |
| R8 | Terminal output report with optimization statistics |
| R9 | Module structure: ingestion/, parser/, scheduler/, renderer/ |
| R10 | Zero external API calls, pure local tree-sitter parsing |

### Non-Functional Goals

- Performance: tree-sitter parsing in milliseconds, parallel file processing via rayon
- Privacy: no network calls, all processing local
- Code quality: clippy pedantic compliance, proptest for token counting and scheduling invariants

### Out of Scope

- Streaming output for very large codebases (future enhancement)
- Caching support for incremental updates (future enhancement)
- Custom output templates / Handlebars (future enhancement)
- Plugin system for custom parsers (future enhancement)
- VS Code extension / GitHub Action (future enhancement)
- MCP server integration (codebank has this, ast-doc defers it)

## Requirements Coverage Matrix

| Req ID | Design Section | Scenarios | Task IDs |
|--------|---------------|-----------|----------|
| R1 | Executive Summary, Detailed Design | generate-llms-txt | 1.1, 2.1, 4.1 |
| R2 | Architecture Overview, Detailed Design | generate-llms-txt | 2.1, 2.2, 2.3, 2.4 |
| R3 | Detailed Design - ingestion/ | discover-source-files, capture-git-context | 2.1 |
| R4 | Detailed Design - parser/ | extract-rust-signatures, strip-test-modules | 2.2 |
| R5 | Detailed Design - scheduler/ | enforce-token-budget, protect-core-files | 2.3 |
| R6 | Detailed Design - renderer/ | generate-llms-txt | 2.4 |
| R7 | Detailed Design - CLI | run-cli-with-options | 1.1, 4.1 |
| R8 | Detailed Design - CLI | show-optimization-report | 4.1 |
| R9 | Architecture Overview | N/A (structural) | 1.1, 1.2 |
| R10 | Architecture Decisions | N/A (architectural) | 2.2 |

## Planner Contract Surface

**PlannedSpecContract:** This spec produces `design.md`, `tasks.md`, and `features/*.feature` under `specs/2026-03-26-01-ast-doc-pipeline/`. The task list covers project identity alignment, four pipeline phases, CLI wiring, and verification.

**TaskContract:** Each task uses `Task X.Y` format with explicit Requirement Coverage, Scenario Coverage, Loop Type, Behavioral Contract, and Verification steps.

**BuildBlockedPacket:** If tree-sitter grammar version conflicts arise during implementation, the build packet should document the incompatibility and suggest version pinning.

**DesignChangeRequestPacket:** If the token scheduler algorithm needs adjustment during implementation, a DCR should update `design.md` Section "Detailed Design - scheduler/" and add corresponding test scenarios.

## Architecture Overview

### System Context

```text
User CLI Input (path, options)
        |
        v
  +-------------------+
  |   CLI (clap)      |  bin/ast-doc
  +-------------------+
        |
        v
  +-------------------+
  |  1. Ingestion     |  Walk dirs, parse .gitignore/.astdocignore
  |     (ignore, git2)|  Capture git metadata, discover files
  +-------------------+
        |
        v
  +-------------------+
  |  2. Parser        |  tree-sitter AST extraction per language
  |     (tree-sitter) |  Apply Full/NoTests/Summary strategy
  +-------------------+
        |
        v
  +-------------------+
  |  3. Scheduler     |  Token budget enforcement
  |     (tiktoken-rs) |  Adaptive degradation algorithm
  +-------------------+
        |
        v
  +-------------------+
  |  4. Renderer      |  Markdown assembly, anti-bloat
  |                   |  llms.txt output
  +-------------------+
        |
        v
  Output: llms.txt file or stdout
```

### Key Design Principles

- **Pipeline architecture**: Each stage is a distinct module with clear input/output types. Stages compose linearly.
- **Strategy pattern**: Parser uses `OutputStrategy` (Full/NoTests/Summary) to control extraction depth per file.
- **Trait-based language dispatch**: `LanguageParser` trait with per-language implementations, factory function for extension-based dispatch.
- **Parallel processing**: File ingestion and AST parsing use rayon for CPU-bound parallelism.

## Architecture Decisions

### Inherited Decisions (from AGENTS.md)

- Rust workspace with `bin/` for CLI crates, `crates/` for library crates
- Error handling: `eyre` for application layer, `thiserror` for library layer
- Logging: `tracing` only, no `println` in library code
- CLI: `clap` with derive feature
- Testing: `cucumber-rs` for BDD, `proptest` for property tests
- Lint: clippy pedantic with documented allow-list
- No `anyhow`, `log`, `reqwest`, `dashmap`

### New Decisions

- **Pattern: Strategy** for parser output modes. Rationale: Full/NoTests/Summary are interchangeable algorithms on the same input (file AST). Strategy pattern avoids conditional branching in the hot path and enables adding new strategies without modifying the parser core. Alternatives rejected: Enum-based dispatch (less extensible), Visitor pattern (over-engineered for 3 strategies).
- **Pattern: Pipeline** for the four stages. Rationale: The design explicitly specifies a linear pipeline. Each stage has a single responsibility (SRP) and communicates through well-defined intermediate types. No circular dependencies exist.
- **DIP: LanguageParser trait** — All language-specific parsers implement a common trait. The scheduler and renderer depend on the trait, not concrete parsers. This enables adding new languages without touching downstream code.
- **Dependency injection:** Git operations are behind a `GitContext` trait so tests can use mock implementations. Token counting uses `tiktoken-rs` directly (no trait needed — it's a stable third-party dependency with no alternative in scope).
- **Code simplification:** Prefer explicit match arms for the 3 strategies over generic trait machinery. The strategy set is small and stable; abstractions beyond a simple enum + match would add ceremony without clarity.
- **Thread-local parser instances for rayon:** `tree_sitter::Parser` requires `&mut self` and is not `Send`. When parsing files in parallel via `rayon::par_iter`, each worker thread must create its own `Parser` instance. Implement this via `thread_local!` storage or by instantiating a fresh parser inside the rayon closure per file. The `LanguageParser` trait methods take `&self` but internal implementations create thread-local parser instances on each call.
- **Feature flags for tree-sitter grammars:** Tree-sitter grammar crates bundle C code and require a C compiler at build time. To avoid slow compile cycles during TDD, make non-primary language grammars optional via Cargo feature flags:

  ```toml
  [features]
  default = ["lang-rust"]
  lang-rust = ["tree-sitter-rust"]
  lang-python = ["tree-sitter-python"]
  lang-typescript = ["tree-sitter-typescript"]
  lang-go = ["tree-sitter-go"]
  lang-c = ["tree-sitter-c"]
  all-languages = ["lang-rust", "lang-python", "lang-typescript", "lang-go", "lang-c"]
  ```

  The parser dispatcher returns an `UnsupportedLanguage` error for disabled languages. CI and release builds use `--all-features`. Development TDD uses `default` (Rust only) for fast iteration.

## BDD/TDD Strategy

- **Primary Language:** Rust
- **BDD Runner:** `cucumber` (cucumber-rs 0.22.1, already in workspace)
- **BDD Command:** `just bdd` (runs `cargo test -p ast-doc --test bdd`)
- **Unit Test Command:** `just test` (runs `cargo test --all-features`)
- **Property Test Tool:** `proptest` (already in workspace)
- **Fuzz Test Tool:** N/A — tree-sitter grammars are trusted inputs, no hostile input parsing in scope
- **Benchmark Tool:** Conditional — `criterion` only if token scheduling latency SLA is defined
- **Feature Files:** `specs/2026-03-26-01-ast-doc-pipeline/features/*.feature`
- **Outside-in Loop:** Start with `generate-llms-txt` scenario (end-to-end), then drive implementation through ingestion -> parser -> scheduler -> renderer

## Code Simplification Constraints

- **Behavioral Contract:** Build new functionality; no existing behavior to preserve (template code is replaced entirely).
- **Repo Standards:** Follow AGENTS.md rules — clippy pedantic, tracing for logging, eyre/thiserror for errors, no unwrap/expect/panic.
- **Readability Priorities:** Explicit match arms for strategy dispatch, clear module boundaries, descriptive type names. Avoid nested closures in the pipeline composition.
- **Refactor Scope:** Replace template code entirely. The `common` crate becomes the ast-doc core library. `cli-app` is renamed to `ast-doc`.
- **Clarity Guardrails:** Avoid one-liner pipeline compositions that obscure data flow. Each pipeline stage should have an explicit function call with named intermediate variables.

## Project Identity Alignment

Current template names to replace:

| Current | Target | Scope |
|---------|--------|-------|
| `bin/cli-app` | `bin/ast-doc` | Directory rename + Cargo.toml name |
| `crates/common` | `crates/ast-doc-core` | Directory rename + Cargo.toml name |
| `features/checkout.feature` | `features/ast-doc.feature` | Replace with ast-doc scenarios |
| `crates/common/tests/bdd.rs` | `crates/ast-doc-core/tests/bdd.rs` | Rewrite step definitions |

## BDD Scenario Inventory

| Feature File | Scenario | Business Outcome |
|-------------|----------|------------------|
| `features/ast-doc.feature` | Generate llms.txt from a project | User runs `ast-doc .` and gets a valid llms.txt with directory tree, git context, and source files |
| `features/ast-doc.feature` | Enforce token budget | User runs `ast-doc . --max-tokens 5000` and output stays within budget via strategy degradation |
| `features/ast-doc.feature` | Protect core files from degradation | Files matching `--core` pattern stay in Full mode even when over budget |
| `features/ast-doc.feature` | Strip test modules in NoTests mode | Test code is removed from output while preserving production code |
| `features/ast-doc.feature` | Extract signatures in Summary mode | Only public signatures are shown, function bodies are omitted |
| `features/ast-doc.feature` | Respect include/exclude patterns | Only matching files appear in output |
| `features/ast-doc.feature` | Report error when budget is insufficient | User gets clear error with actionable suggestion when even minimum strategies exceed budget |

## Detailed Design

### Module Structure

```text
crates/ast-doc-core/src/
├── lib.rs               # Public API, re-exports
├── config.rs            # AstDocConfig struct
├── error.rs             # Error types (thiserror)
├── ingestion/           # Phase 1: File discovery
│   ├── mod.rs           # IngestionResult, run_ingestion()
│   ├── walker.rs        # Directory traversal with ignore crate
│   ├── filter.rs        # .gitignore/.astdocignore + glob include/exclude
│   └── git.rs           # GitContext trait + Git2Context impl
├── parser/              # Phase 2: AST extraction
│   ├── mod.rs           # LanguageParser trait, parse_file()
│   ├── lang/
│   │   ├── mod.rs       # Parser registry, detect_language()
│   │   ├── rust.rs      # RustParser
│   │   ├── python.rs    # PythonParser
│   │   ├── typescript.rs # TypeScriptParser
│   │   ├── go.rs        # GoParser
│   │   └── c.rs         # CParser
│   └── strategy.rs      # OutputStrategy enum, apply_strategy()
├── scheduler/           # Phase 3: Token budget
│   ├── mod.rs           # ScheduleResult, run_scheduler()
│   └── optimizer.rs     # Degradation algorithm
└── renderer/            # Phase 4: Output
    ├── mod.rs           # render_llms_txt()
    └── llms_txt.rs      # Markdown assembly, anti-bloat

bin/ast-doc/src/
└── main.rs              # CLI entry point, clap args, pipeline orchestration
```

### Key Data Types

```rust
// config.rs
pub struct AstDocConfig {
    pub path: PathBuf,
    pub output: Option<PathBuf>,
    pub max_tokens: usize,
    pub core_patterns: Vec<String>,
    pub default_strategy: OutputStrategy,
    pub include_patterns: Vec<String>,
    pub exclude_patterns: Vec<String>,
    pub no_git: bool,
    pub no_tree: bool,
    pub copy: bool,
    pub verbose: bool,
}

// ingestion/mod.rs
pub struct IngestionResult {
    pub files: Vec<DiscoveredFile>,
    pub directory_tree: String,
    pub git_context: Option<GitContext>,
}

pub struct DiscoveredFile {
    pub path: PathBuf,
    pub content: String,
    pub language: Option<Language>,
    pub raw_token_count: usize,
}

// parser/mod.rs
pub trait LanguageParser {
    fn parse(&self, source: &str, path: &Path) -> Result<ParsedFile>;
}

pub struct ParsedFile {
    pub path: PathBuf,
    pub language: Language,
    pub source: String,
    /// Pre-computed strategy data for each output mode.
    /// Phase 2 generates all three variants so Phase 3 (Scheduler)
    /// can make purely mathematical decisions without touching strings.
    pub strategies_data: HashMap<OutputStrategy, StrategyData>,
}

/// Pre-computed content and token count for a single strategy.
pub struct StrategyData {
    /// The rendered source text for this strategy.
    pub content: String,
    /// Token count of content (computed once via tiktoken-rs during parsing).
    pub token_count: usize,
}

/// Byte ranges to remove from the original source.
/// Used internally by NoTests and Summary strategies.
pub struct RemovalRange {
    pub start: usize,
    pub end: usize,
    pub reason: RemovalReason,
}

pub enum RemovalReason {
    TestModule,
    TestFunction,
    FunctionBody,
}

// parser/strategy.rs
pub enum OutputStrategy {
    Full,
    NoTests,
    Summary,
}

// scheduler/mod.rs
pub struct ScheduleResult {
    pub files: Vec<ScheduledFile>,
    pub total_tokens: usize,
    pub raw_tokens: usize,
    pub strategy_counts: HashMap<OutputStrategy, usize>,
}

pub struct ScheduledFile {
    pub parsed: ParsedFile,
    pub strategy: OutputStrategy,
    pub rendered_tokens: usize,
    pub saved_tokens: usize,
}
```

### Pipeline Flow

```rust
// lib.rs - main pipeline
pub fn run_pipeline(config: &AstDocConfig) -> Result<String> {
    // Phase 1: Ingestion — file discovery, git metadata, directory tree
    let ingestion = ingestion::run_ingestion(config)?;

    // Phase 2: Parser — tree-sitter extraction + pre-compute all strategy variants
    // Each ParsedFile contains strategies_data[Full/NoTests/Summary] with
    // pre-rendered content and token counts. This makes Phase 3 purely mathematical.
    let parsed: Vec<ParsedFile> = ingestion.files.par_iter()
        .filter_map(|f| f.language.map(|lang| (f, lang)))
        .map(|(f, lang)| parser::parse_file(f, lang))
        .collect::<Result<Vec<_>>>()?;

    // Phase 3: Scheduler — pure optimization using pre-computed token counts.
    // No string manipulation. Selects strategy per file to fit budget.
    let scheduled = scheduler::run_scheduler(&parsed, config)?;

    // Phase 4: Renderer — assemble final markdown from selected strategy content.
    let output = renderer::render_llms_txt(&scheduled, &ingestion, config)?;

    Ok(output)
}
```

### AST Extraction Approach: Byte-Range Slicing

The parser uses **byte-range slicing** (not structural reconstruction) to preserve full source fidelity:

- **Full mode**: Output the original `source` string verbatim. Zero transformation — preserves license headers, macros, formatting, comments, and all non-structural code.
- **NoTests mode**: Use tree-sitter to identify byte ranges of test modules (`#[cfg(test)]` blocks) and test functions (`#[test]`/`func Test*`/`def test_*`). Remove those byte ranges from the original source and insert `// ✂️ test module omitted` markers. All non-test code remains 100% intact.
- **Summary mode**: Use tree-sitter to extract function/struct/trait signatures. Build output from extracted signatures only, with `// ✂️ implementations omitted` markers. Doc comments are preserved.

This approach avoids the fidelity loss of reconstructing source from structured `CodeUnit` representations, which would drop license headers, standalone macros, and original formatting. The `CodeUnit` enum remains useful for internal metadata (test detection, signature extraction, token counting) but is not the output format.

### Token Scheduler Algorithm

```text
Input: parsed files (with pre-computed strategy token counts), max_tokens, core_patterns

1. Compute base_overhead = token_count(directory_tree + git_context)
   - If git diff exceeds 1000 tokens, truncate with "… (diff truncated)" suffix
   - If base_overhead >= max_tokens, return BudgetExceededError("Metadata alone
     exceeds token budget. Increase --max-tokens or use --no-git / --no-tree.")
   - remaining_budget = max_tokens - base_overhead

2. For each file:
   - If matches core_patterns -> lock strategy = Full
   - Else -> start with default_strategy (Full by default)

3. Calculate total_tokens = sum(file.strategies_data[assigned_strategy].token_count)

4. Degradation loop (while total_tokens > remaining_budget):
   a. Collect degradable files (non-core, strategy > Summary)
   b. If no degradable files remain:
      - All non-core files are at Summary, but budget still exceeded.
      - Return BudgetExceededError("All files at minimum strategy but still
        exceeding budget. Consider --no-git or reducing source scope.")
   c. Sort degradable files by priority:
      - Files with test-heavy content first (NoTests saves more tokens)
      - Then by current strategy token count descending (largest savings first)
   d. Degrade one step: Full -> NoTests, or NoTests -> Summary
   e. Recalculate total_tokens
   f. Safety: if no token reduction occurred after a degradation step,
      force-skip the file (treat as Omit) to prevent infinite loop.

5. Return ScheduleResult with final strategy per file
```

The scheduler is a **pure mathematical optimizer** — it selects from pre-computed `strategies_data` entries without ever manipulating strings. This eliminates the chicken-and-egg problem where the scheduler would need to render text to know token counts.

### Test Detection Rules

| Language | Function-level test markers | Module-level test markers |
|----------|---------------------------|--------------------------|
| Rust | `#[test]`, `#[tokio::test]` | `#[cfg(test)]`, module named `tests` |
| Python | `def test_*`, `@pytest.mark.*` | `class Test*` |
| TypeScript | `it(`, `test(`, `describe(` | test files `*.test.*`, `*.spec.*` |
| Go | `func Test*`, `func Benchmark*` | `_test.go` suffix |
| C | No standard markers | No standard markers |

### llms.txt Output Structure

```markdown
# Repository: {Project_Name}

> {Description from first line of README or Cargo.toml description}
> Note: This codebase has been optimized using AST trimming to fit token limits.
> Files are presented in Full, NoTests, or Summary (signatures only) modes.

## Structure & Symbol Index

### Directory Tree

{tree with [Full]/[NoTests]/[Summary] annotations}

### Git Context

- **Branch**: {branch_name}
- **Latest Commit**: {commit_summary}
- **Uncommitted Changes**: {diff_summary}

---

## Source Files

### File: {path}

*Strategy: {mode} | Tokens: {count} (Saved: {saved})*

{code block with content based on strategy}
```

## Verification & Testing Strategy

### Unit Tests (colocated with implementation)

- `parser/lang/rust.rs`: Test extraction of functions, structs, traits, impls, enums from Rust source
- `parser/lang/python.rs`: Test extraction of functions, classes from Python source
- `parser/strategy.rs`: Test NoTests stripping and Summary signature extraction per language
- `scheduler/optimizer.rs`: Test degradation algorithm with various budget/file combinations
- `renderer/llms_txt.rs`: Test markdown assembly and anti-bloat rules

### Property Tests (proptest)

- **Scheduler invariant:** `total_tokens <= max_tokens` for any input set after scheduling
- **Parser invariant:** `parse(source)` always produces strategy content whose characters are a subset (or exact copy) of the input source
- **Token counting (BPE superadditivity):** `count_tokens(a + b) <= count_tokens(a) + count_tokens(b)` — BPE tokenizers may merge boundary characters between two separately-tokenized strings into fewer tokens when concatenated. The correct invariant is superadditivity (concatenated count is less than or equal to the sum of individual counts), not subadditivity.

### BDD Scenarios

- End-to-end: run ast-doc on a fixture project, verify llms.txt structure
- Token budget: verify output respects --max-tokens limit
- Core protection: verify --core files never degrade
- Strategy correctness: verify test stripping and signature extraction

### Fuzz/Benchmark

- **Fuzz:** N/A — tree-sitter grammars are trusted, no hostile input parsing
- **Benchmark:** Conditional — add criterion benchmarks for `scheduler::run_scheduler` if performance regression risk is identified during implementation

## Implementation Plan

1. **Phase 1: Project Identity** — Rename cli-app -> ast-doc, common -> ast-doc-core, replace template code
2. **Phase 2: Core Pipeline** — Implement ingestion, parser, scheduler, renderer modules
3. **Phase 3: CLI Wiring** — Wire clap CLI to pipeline, add terminal report
4. **Phase 4: Verification** — BDD scenarios, property tests, integration testing

## Revision History

| Date | Change | Reason |
| :--- | :--- | :--- |
| 2026-03-26 | Redesigned parser to byte-range slicing with pre-computed strategy data; Scheduler now purely mathematical | Code review: AST fidelity loss if reconstructing from CodeUnits; chicken-and-egg problem where Scheduler needs token counts before Renderer runs |
| 2026-03-26 | Added Scheduler termination conditions (BudgetExceededError), git diff truncation, infinite-loop safety valve | Code review: missing boundary handling when all files at Summary but still over budget; git diff may explode base_overhead |
| 2026-03-26 | Fixed proptest invariant from subadditivity to superadditivity for BPE token counting | Code review: BPE merges boundary tokens on concatenation, so count(a+b) <= count(a) + count(b) |
| 2026-03-26 | Added thread-local parser strategy for rayon compatibility | Code review: tree_sitter::Parser is not Send; par_iter requires per-thread instances |
| 2026-03-26 | Added feature flags for tree-sitter language grammars | Code review: tree-sitter C compilation is slow; feature flags enable fast TDD with Rust-only default |
