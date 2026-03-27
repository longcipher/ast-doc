# ast-doc Skill

## Description

AST-powered code documentation tool for generating optimized `llms.txt` files from codebases. Use this skill when users need to:

- Generate LLM-optimized documentation from their codebase
- Reduce token usage when feeding code to AI models
- Create structured Markdown summaries of code repositories
- Extract code architecture and public interfaces
- Prepare code context for AI-assisted development

Triggers include: "generate llms.txt", "document codebase", "create code summary", "ast documentation", "optimize code for LLM", "code2prompt", "codebank", or any request to prepare code for AI consumption.

## Usage

The ast-doc tool combines broad file traversal with deep AST-based semantic parsing to create optimized documentation.

### Basic Command

```bash
ast-doc <path> [OPTIONS]
```

### Common Use Cases

1. **Generate basic documentation**:

   ```bash
   ast-doc .
   ```

2. **Strict token limit with output file**:

   ```bash
   ast-doc . --max-tokens 30000 --output llms.txt
   ```

3. **Summary mode for quick overview**:

   ```bash
   ast-doc . --strategy summary --output summary.md
   ```

4. **Protect core files from degradation**:

   ```bash
   ast-doc . --max-tokens 50000 --core "src/core/**" --output docs.txt
   ```

5. **Copy to clipboard**:

   ```bash
   ast-doc . --copy
   ```

### Options

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

### Output Strategies

1. **Full Mode**: Complete source code preservation
2. **NoTests Mode**: Removes test modules and test functions
3. **Summary Mode**: Extracts only public interfaces, signatures, and docstrings

### Example Workflow

When a user asks to document their codebase for AI consumption:

1. Check if ast-doc is installed:

   ```bash
   which ast-doc
   ```

2. If not installed, install via cargo:

   ```bash
   cargo install ast-doc
   ```

3. Run ast-doc with appropriate options based on user requirements:

   ```bash
   ast-doc /path/to/project --max-tokens 50000 --output llms.txt
   ```

4. Review the generated report showing optimization statistics

### Output Format

The tool generates a standard `llms.txt` Markdown file containing:

- Repository name and description
- Directory tree with strategy annotations
- Git context (branch, commits, changes)
- Source files with their processing strategy and token counts

### Integration with Code Agents

This tool is designed to be used by code agents to:

- Prepare code context before making API calls to LLMs
- Reduce token costs by 60-80% while preserving architectural information
- Generate consistent, well-structured documentation across projects
- Enable efficient code review and analysis workflows

## Tips

- Use `--core` to protect critical files from being summarized
- Start with higher `--max-tokens` values and decrease as needed
- Use `--strategy summary` for quick architectural overviews
- Combine with `--no-git` and `--no-tree` for pure code output
- Check the optimization report to understand token savings
