---
name: ast-doc
description: AST-powered code documentation tool for generating optimized llms.txt files from codebases. Use this skill when users need to generate LLM-optimized documentation, reduce token usage when feeding code to AI models, create structured Markdown summaries of code repositories, extract code architecture and public interfaces, or prepare code context for AI-assisted development.
metadata:
  author: longcipher
  version: "0.1.0"
---

# AST Documentation Generator

Generate optimized `llms.txt` files from codebases using AST-based semantic parsing.

## Triggers

Use this skill when users ask to:

- Generate llms.txt or LLM-optimized documentation
- Document a codebase for AI consumption
- Create code summaries or architecture overviews
- Optimize code context for AI models
- Use code2prompt or similar tools

## Installation

Install the skill:

```bash
npx skills add longcipher/ast-doc
```

Install the CLI tool:

```bash
cargo install ast-doc
```

## Basic Usage

```bash
# Generate documentation from current directory
ast-doc .

# With token limit and output file
ast-doc . --max-tokens 30000 --output llms.txt

# Summary mode for quick overview
ast-doc . --strategy summary --output summary.md

# Protect core files from degradation
ast-doc . --max-tokens 50000 --core "src/core/**" --output docs.txt

# Copy to clipboard
ast-doc . --copy
```

## Options

| Option | Description |
|--------|-------------|
| `-o, --output <FILE>` | Output file path (default: stdout) |
| `-m, --max-tokens <NUM>` | Maximum token budget (default: 100000) |
| `-c, --core <PATTERN>` | Glob pattern for core files (never degrade) |
| `-s, --strategy <STRATEGY>` | Default strategy: `full`, `no-tests`, or `summary` |
| `--include <PATTERN>` | Include file patterns |
| `--exclude <PATTERN>` | Exclude file patterns |
| `--no-git` | Skip git context |
| `--no-tree` | Skip directory tree |
| `--copy` | Copy output to clipboard |
| `-v, --verbose` | Verbose output |

## Output Strategies

1. **Full Mode**: Complete source code preservation
2. **NoTests Mode**: Removes test modules and test functions
3. **Summary Mode**: Extracts only public interfaces, signatures, and docstrings

## Workflow

1. Check if ast-doc is installed:

   ```bash
   which ast-doc
   ```

2. If not installed, install via cargo:

   ```bash
   cargo install ast-doc
   ```

3. Run ast-doc with appropriate options:

   ```bash
   ast-doc /path/to/project --max-tokens 50000 --output llms.txt
   ```

4. Review the generated report showing optimization statistics

## Output Format

The tool generates a standard `llms.txt` Markdown file containing:

- Repository name and description
- Directory tree with strategy annotations
- Git context (branch, commits, changes)
- Source files with their processing strategy and token counts

## Tips

- Use `--core` to protect critical files from being summarized
- Start with higher `--max-tokens` values and decrease as needed
- Use `--strategy summary` for quick architectural overviews
- Combine with `--no-git` and `--no-tree` for pure code output
- Check the optimization report to understand token savings
