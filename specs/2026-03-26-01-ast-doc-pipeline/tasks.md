# ast-doc Pipeline — Tasks

| Metadata | Details |
| :--- | :--- |
| **Design Doc** | specs/2026-03-26-01-ast-doc-pipeline/design.md |
| **Status** | Planning |

## Summary & Timeline

| Phase | Focus | Tasks | Dependency |
|-------|-------|-------|------------|
| Phase 1 | Project Identity Alignment | Task 1.1, Task 1.2 | None |
| Phase 2 | Core Pipeline Implementation | Task 2.1, Task 2.2, Task 2.3, Task 2.4 | Phase 1 |
| Phase 3 | CLI Wiring & Reporting | Task 3.1, Task 3.2 | Phase 2 |
| Phase 4 | Verification & Testing | Task 4.1, Task 4.2 | Phase 3 |

## Definition of Done

- `just lint` passes (clippy pedantic, typos, taplo, rustfmt)
- `just test` passes (all unit + property tests)
- `just bdd` passes (all Gherkin scenarios green)
- `just build` passes (workspace compiles cleanly)
- `just test-all` passes (TDD + BDD combined)

---

## Phase 1: Project Identity Alignment

### Task 1.1: Rename CLI binary crate from cli-app to ast-doc

> **Context:** The repo is scaffold-derived. `bin/cli-app` is a template hello-world binary. Rename to `bin/ast-doc` and update its Cargo.toml to use the name `ast-doc`. Update the root workspace `Cargo.toml` if needed (it uses `bin/*` glob, so no change needed there). Replace the main.rs with a minimal clap entry point that accepts the core CLI arguments from the design doc.
> **Requirement Coverage:** R7, R9
> **Scenario Coverage:** N/A (structural, no user-visible behavior yet)

- **Loop Type:** `TDD-only`
- **Behavioral Contract:** Replace template behavior entirely with ast-doc CLI skeleton.
- **Simplification Focus:** Minimal viable CLI — only the clap struct and a placeholder `main()` that calls the pipeline.
- **Advanced Test Coverage:** Example-based only
- **Status:** 🟢 DONE
- [x] Rename directory `bin/cli-app` to `bin/ast-doc`
- [x] Update `bin/ast-doc/Cargo.toml`: change `name = "cli-app"` to `name = "ast-doc"`, update description
- [x] Replace `bin/ast-doc/src/main.rs` with clap CLI struct matching design doc options (path, output, max-tokens, core, strategy, include, exclude, no-git, no-tree, copy, verbose)
- [x] Add dependencies: `eyre`, `tracing`, `tracing-subscriber`, `common` (placeholder for `ast-doc-core`)
- [x] Wire main.rs with placeholder output (pipeline call deferred to Task 1.2)
- [x] Verification: `cargo check -p ast-doc` compiles

### Task 1.2: Replace common crate with ast-doc-core

> **Context:** `crates/common` contains template shopping cart code. Replace with `crates/ast-doc-core` — the core library that will hold the four-stage pipeline. Remove the template Cart/Order/checkout_cart code and replace with the module structure from the design doc.
> **Requirement Coverage:** R9
> **Scenario Coverage:** N/A (structural)

- **Loop Type:** `TDD-only`
- **Behavioral Contract:** Replace template behavior entirely. No existing behavior to preserve.
- **Simplification Focus:** Clean module skeleton with pub stubs for each pipeline stage.
- **Advanced Test Coverage:** Example-based only
- **Status:** 🟢 DONE
- [x] Rename directory `crates/common` to `crates/ast-doc-core`
- [x] Update `crates/ast-doc-core/Cargo.toml`: change `name = "ast-doc-core"`, add workspace dependencies, feature flags for tree-sitter grammars
- [x] Add workspace-level dependency entries in root `Cargo.toml`
- [x] Create module skeleton in `crates/ast-doc-core/src/`: `lib.rs`, `config.rs`, `error.rs`, `ingestion/mod.rs`, `parser/mod.rs`, `scheduler/mod.rs`, `renderer/mod.rs`
- [x] Update `bin/ast-doc/Cargo.toml` dependency from `common` to `ast-doc-core`
- [x] Replace `features/checkout.feature` with `features/ast-doc.feature`
- [x] Update `crates/ast-doc-core/tests/bdd.rs` to reference ast-doc-core types
- [x] Verification: `cargo check --workspace` compiles

---

## Phase 2: Core Pipeline Implementation

### Task 2.1: Implement Ingestion Module

> **Context:** Phase 1 of the pipeline. Implements file discovery with .gitignore/.astdocignore parsing, git metadata capture, and directory tree generation. Reuse patterns from `code2prompt/crates/code2prompt-core/src/path.rs` (directory walking with `ignore` crate) and `git.rs` (git2 integration).
> **Requirement Coverage:** R3
> **Scenario Coverage:** discover-source-files, capture-git-context
> **Reusable Components:** `code2prompt` patterns for `ignore::WalkBuilder` usage, `git2` diff/log operations, `termtree` for directory tree rendering.

- **Loop Type:** `BDD+TDD`
- **Behavioral Contract:** New behavior — discovers source files respecting .gitignore/.astdocignore, captures git context, produces directory tree.
- **Simplification Focus:** Clear separation of walker, filter, and git concerns. No unnecessary abstraction layers.
- **Advanced Test Coverage:** Example-based only
- **Status:** 🟢 DONE
- [x] Implement `ingestion/walker.rs`: directory traversal using `ignore::WalkBuilder` with .gitignore + .astdocignore support, glob-based include/exclude filtering via `globset`
- [x] Implement `ingestion/git.rs`: `GitContext` struct (branch, latest commit, uncommitted changes), `GitContextProvider` trait for testability, `Git2Context` implementation using `git2` crate
- [x] Implement `ingestion/mod.rs`: `IngestionResult` struct, `run_ingestion(config) -> Result<IngestionResult>` that orchestrates walker + git, builds directory tree string
- [x] Add `config.rs`: `AstDocConfig` struct with all CLI fields, `OutputStrategy` enum
- [x] Add `error.rs`: `AstDocError` enum using `thiserror`
- [x] Unit tests: 32 tests passing (walker, git, ingestion)
- [ ] BDD Verification: deferred to Task 4.1 (step definitions not yet implemented)
- [x] Verification: `cargo test -p ast-doc-core` passes (32 tests)

### Task 2.2: Implement AST Parser Module

> **Context:** Phase 2 of the pipeline. Implements multi-language tree-sitter parsing with Full/NoTests/Summary strategies. Uses byte-range slicing (not structural reconstruction) to preserve full source fidelity. Each ParsedFile contains pre-computed `strategies_data` (HashMap<OutputStrategy, StrategyData>) with rendered content and token counts for all three modes, making Phase 3 (Scheduler) a purely mathematical optimizer. Reuse patterns from `codebank/src/parser/` (LanguageParser trait, per-language implementations, test detection).
> **Requirement Coverage:** R4, R10
> **Scenario Coverage:** extract-rust-signatures, strip-test-modules
> **Reusable Components:** `codebank` patterns for `LanguageParser` trait design, tree-sitter node walking, test marker detection, signature extraction.

- **Loop Type:** `BDD+TDD`
- **Behavioral Contract:** New behavior — parses source files, identifies byte ranges for test removal and signature extraction, pre-computes all three strategy variants with token counts.
- **Simplification Focus:** One parser file per language. Full mode = verbatim source (zero transformation). NoTests = remove byte ranges of test code. Summary = extract signatures only. No generic trait machinery beyond `LanguageParser`.
- **Advanced Test Coverage:** Property tests for parser invariants (strategy content is subset of input source)
- **Status:** 🟢 DONE
- [x] Add workspace dependencies and feature flags in `crates/ast-doc-core/Cargo.toml`
- [x] Implement `parser/mod.rs`: `parse_file()` dispatcher, `detect_language()`, `RemovalRange`/`RemovalReason`
- [x] Implement `parser/strategy.rs`: byte-range slicing engine with Full/NoTests/Summary, token counting via tiktoken-rs
- [x] Implement `parser/lang/rust_parser.rs`: `RustParser` with tree-sitter, test detection, signature extraction
- [x] Implement `parser/lang/python_parser.rs`: `PythonParser` with test markers
- [x] Implement `parser/lang/typescript_parser.rs`: `TypeScriptParser` with test markers
- [x] Implement `parser/lang/go_parser.rs`: `GoParser` with test markers
- [x] Implement `parser/lang/c_parser.rs`: `CParser` basic extraction
- [x] Unit tests per language: 92 tests pass with --all-features
- [ ] Property tests: deferred to Task 4.2
- [ ] BDD Verification: deferred to Task 4.1
- [x] Verification: `cargo test -p ast-doc-core` passes

### Task 2.3: Implement Token Scheduler

> **Context:** Phase 3 of the pipeline. Pure mathematical optimizer that selects from pre-computed `strategies_data` entries (HashMap<OutputStrategy, StrategyData>) per ParsedFile. No string manipulation — just token arithmetic. Enforces `--max-tokens` budget by degrading file strategies from Full -> NoTests -> Summary, while protecting core files.
> **Requirement Coverage:** R5
> **Scenario Coverage:** enforce-token-budget, protect-core-files, report-budget-exceeded
> **Reusable Components:** `code2prompt` patterns for `tiktoken-rs` token counting.

- **Loop Type:** `BDD+TDD`
- **Behavioral Contract:** New behavior — schedules file strategies to fit within token budget. Returns `BudgetExceededError` when even minimum strategies exceed budget.
- **Simplification Focus:** Single optimizer function with clear step-by-step algorithm. Pure math over pre-computed data — no rendering or string work.
- **Advanced Test Coverage:** Property tests for budget invariant (total_tokens <= max_tokens after scheduling)
- **Status:** 🟢 DONE
- [x] Add workspace dependency: `tiktoken-rs`
- [x] Implement `scheduler/mod.rs`: `ScheduledFile`, `ScheduleResult`, `run_scheduler()` with base_overhead_tokens param
- [x] Implement `scheduler/optimizer.rs`: degradation algorithm with all safeguards (base_overhead check, BudgetExceededError, safety valve, test-heavy sorting)
- [x] Unit tests: 13 optimizer tests covering budget scenarios, core protection, edge cases
- [ ] Property tests: deferred to Task 4.2
- [ ] BDD Verification: deferred to Task 4.1
- [x] Verification: `cargo test -p ast-doc-core` passes (75 tests)

### Task 2.4: Implement Renderer

> **Context:** Phase 4 of the pipeline. Assembles the final `llms.txt` markdown output from scheduled files, directory tree, and git context. Applies anti-bloat rules (compress blank lines, remove excess whitespace).
> **Requirement Coverage:** R6, R8
> **Scenario Coverage:** generate-llms-txt

- **Loop Type:** `BDD+TDD`
- **Behavioral Contract:** New behavior — renders structured markdown output per the llms.txt specification.
- **Simplification Focus:** Simple string assembly with format! macros. No template engine needed — the output format is fixed and well-specified.
- **Advanced Test Coverage:** Example-based only
- **Status:** 🟢 DONE
- [x] Implement `renderer/mod.rs`: `render_llms_txt(scheduled, ingestion, config) -> Result<String>` entry point
- [x] Implement `renderer/llms_txt.rs`: markdown assembly with header, structure, source files sections
- [x] Implement anti-bloat rules: compress consecutive blank lines, trim trailing whitespace
- [x] Implement strategy annotations in code blocks
- [x] Unit tests: 14 renderer tests covering output structure, anti-bloat, strategy annotations
- [ ] BDD Verification: deferred to Task 4.1
- [x] Verification: `cargo test -p ast-doc-core` passes (89 tests)

---

## Phase 3: CLI Wiring & Reporting

### Task 3.1: Wire CLI to Pipeline

> **Context:** Connect the clap CLI from Task 1.1 to the pipeline from Tasks 2.1-2.4. The CLI should parse arguments, build `AstDocConfig`, call `run_pipeline()`, and write output to file or stdout.
> **Requirement Coverage:** R7
> **Scenario Coverage:** run-cli-with-options

- **Loop Type:** `BDD+TDD`
- **Behavioral Contract:** New behavior — full CLI integration.
- **Simplification Focus:** Straightforward argument-to-config mapping. No complex CLI subcommand logic.
- **Advanced Test Coverage:** Example-based only
- **Status:** 🟢 DONE
- [x] Implement `bin/ast-doc/src/main.rs`: parse clap Args, build `AstDocConfig`, call `ast_doc_core::run_pipeline()`, write output to file or stdout
- [x] Add `--copy` support: warning logged (clipboard not yet implemented)
- [x] Add `--verbose` support: tracing-subscriber with appropriate level
- [x] Handle errors with `eyre`: user-friendly error messages with context
- [x] Unit tests: 5 tests for argument parsing, config building, strategy conversion
- [ ] BDD Verification: deferred to Task 4.1
- [x] Verification: `cargo test -p ast-doc` passes (5 tests), `cargo run -p ast-doc -- --help` shows correct options

### Task 3.2: Implement Terminal Report

> **Context:** Print the optimization report to stderr after generation completes. Shows file counts, token breakdown by strategy, and savings percentage. Matches the terminal output format specified in the design doc.
> **Requirement Coverage:** R8
> **Scenario Coverage:** show-optimization-report

- **Loop Type:** `BDD+TDD`
- **Behavioral Contract:** New behavior — displays human-readable optimization summary to terminal.
- **Simplification Focus:** Simple format! output to stderr. No rich TUI needed.
- **Advanced Test Coverage:** Example-based only
- **Status:** 🟢 DONE
- [x] Implement report generation in `bin/ast-doc/src/report.rs`: format ScheduleResult into terminal report
- [x] Print to stderr: total files, raw tokens, final tokens, strategy breakdown, savings percentage
- [x] Budget status indicators: 🟢/🔴 for budget status
- [x] Unit tests: 5 tests for report formatting
- [ ] BDD Verification: deferred to Task 4.1
- [x] Verification: `cargo test -p ast-doc` passes (10 tests)

---

## Phase 4: Verification & Testing

### Task 4.1: End-to-End BDD Scenarios

> **Context:** Write and verify all Gherkin scenarios for the ast-doc feature. Place feature file at `features/ast-doc.feature` and step definitions in `crates/ast-doc-core/tests/bdd.rs`.
> **Requirement Coverage:** R1, R2, R7, R8
> **Scenario Coverage:** All scenarios in features/ast-doc.feature
> **Reusable Components:** Existing cucumber-rs BDD harness structure from `crates/common/tests/bdd.rs` (adapted to ast-doc-core).

- **Loop Type:** `BDD+TDD`
- **Behavioral Contract:** New behavior — end-to-end acceptance tests.
- **Simplification Focus:** Thin step definitions that delegate to library functions. Business logic stays in ast-doc-core, not in test code.
- **Advanced Test Coverage:** Example-based only
- **Status:** 🟢 DONE
- [x] Feature file `features/ast-doc.feature` with all 7 scenarios
- [x] Fixture test projects created via tempfile in BDD steps
- [x] Step definitions in `crates/ast-doc-core/tests/bdd.rs`: 35 steps across 7 scenarios
- [x] Verification: all 7 scenarios pass (35 steps)

### Task 4.2: Property Tests for Scheduler and Parser

> **Context:** Add proptest coverage for the two components with broad input domains: the token scheduler (budget invariant) and the AST parser (strategy content subset invariant). Token counting uses BPE superadditivity: `count_tokens(a + b) <= count_tokens(a) + count_tokens(b)`.
> **Requirement Coverage:** R4, R5
> **Scenario Coverage:** enforce-token-budget, extract-rust-signatures

- **Loop Type:** `TDD-only`
- **Behavioral Contract:** New behavior — property-based test coverage.
- **Simplification Focus:** Colocate property tests with the modules they test (scheduler/optimizer.rs tests, parser/lang/rust.rs tests).
- **Advanced Test Coverage:** Property (proptest)
- **Status:** 🟢 DONE
- [x] Property tests in `scheduler/optimizer.rs`: budget_invariant, core_files_always_full, bpe_tokenizer_invariants
- [x] Property tests in `parser/lang/rust_parser.rs`: parser_content_subset_invariant
- [x] Property tests in `renderer/llms_txt.rs`: anti_bloat_no_consecutive_blank_lines
- [x] Verification: `cargo test -p ast-doc-core` passes (94 tests including 5 proptest cases)
