//! llms.txt markdown assembly.
//!
//! Assembles the final markdown output from scheduled files, directory tree,
//! and git context. Applies anti-bloat rules (compress blank lines, trim
//! trailing whitespace).

use std::collections::HashMap;

use crate::{
    config::OutputStrategy,
    ingestion::IngestionResult,
    parser::Language,
    scheduler::{ScheduleResult, ScheduledFile},
};

/// Render the llms.txt markdown from scheduled files and ingestion data.
///
/// Applies anti-bloat rules:
/// - Compress consecutive blank lines to a single blank line
/// - Trim trailing whitespace per line
#[must_use]
pub fn render(
    scheduled: &ScheduleResult,
    ingestion: &IngestionResult,
    project_name: &str,
    description: &str,
) -> String {
    let mut buf = String::new();

    // Header
    buf.push_str(&format!("# Repository: {project_name}\n\n"));

    // Description blockquote
    if !description.is_empty() {
        buf.push_str(&format!("> {description}\n"));
    }

    // Build accurate mode description based on actual strategies used
    let mut used_modes = Vec::new();
    if scheduled.strategy_counts.contains_key(&OutputStrategy::Full) {
        used_modes.push("Full");
    }
    if scheduled.strategy_counts.contains_key(&OutputStrategy::NoTests) {
        used_modes.push("NoTests");
    }
    if scheduled.strategy_counts.contains_key(&OutputStrategy::Summary) {
        used_modes.push("Summary (signatures only)");
    }

    buf.push_str(
        "> Note: This codebase has been optimized using AST trimming to fit token limits.\n",
    );
    if used_modes.is_empty() {
        buf.push_str("> No source files were included in the output.\n\n");
    } else {
        let modes_str = used_modes.join(", ");
        buf.push_str(&format!(
            "> Files are presented in {modes_str} mode{}.\n\n",
            if used_modes.len() == 1 { "" } else { "s" }
        ));
    }

    // Structure & Symbol Index
    buf.push_str("## Structure & Symbol Index\n\n");

    // Directory Tree
    if !ingestion.directory_tree.is_empty() {
        buf.push_str("### Directory Tree\n\n");
        buf.push_str(&annotate_directory_tree(&ingestion.directory_tree, &scheduled.files));
        buf.push('\n');
    }

    // Git Context
    if let Some(ref git) = ingestion.git_context {
        buf.push_str("### Git Context\n\n");
        buf.push_str(&format!("- **Branch**: {}\n", git.branch));
        buf.push_str(&format!("- **Latest Commit**: {}\n", git.latest_commit));
        if let Some(ref diff) = git.diff {
            buf.push_str(&format!("- **Uncommitted Changes**: {}\n", diff));
        }
        buf.push('\n');
    }

    // Source Files separator
    buf.push_str("---\n\n");
    buf.push_str("## Source Files\n\n");

    // Render each file
    for file in &scheduled.files {
        render_file(&mut buf, file);
    }

    // Apply anti-bloat rules
    apply_anti_bloat(&buf)
}

/// Annotate the directory tree with strategy labels for each file.
fn annotate_directory_tree(tree: &str, files: &[ScheduledFile]) -> String {
    let strategy_map: HashMap<String, &OutputStrategy> =
        files.iter().map(|f| (f.parsed.path.to_string_lossy().to_string(), &f.strategy)).collect();

    let mut result = String::new();
    for line in tree.lines() {
        let trimmed = line.trim_end();
        // Check if this line is a file leaf (has language annotation like [Rust])
        let annotated = annotate_tree_line(trimmed, &strategy_map);
        result.push_str(&annotated);
        result.push('\n');
    }
    result
}

/// Try to annotate a single tree line with the strategy if it matches a file.
fn annotate_tree_line(line: &str, strategy_map: &HashMap<String, &OutputStrategy>) -> String {
    // Extract the filename from the tree line — it's the last segment
    // Lines look like: "│   ├── main.rs [Rust]" or "├── lib.rs [Python]"
    // We need to match the filename portion against our strategy map.
    for (path, strategy) in strategy_map {
        let file_name = path.rsplit('/').next().unwrap_or(path);
        if line.contains(file_name) && !line.contains('[') || line.contains(file_name) {
            // Replace language annotation with strategy, or append strategy
            // Check if line already has a bracket annotation (language)
            if let Some(bracket_pos) = line.rfind('[') {
                // Replace the language annotation with strategy
                let prefix = &line[..bracket_pos];
                return format!("{prefix}[{strategy}]");
            }
            return format!("{line} [{strategy}]");
        }
    }
    line.to_string()
}

/// Render a single file section into the buffer.
fn render_file(buf: &mut String, file: &ScheduledFile) {
    let path_display = file.parsed.path.display();
    let strategy = &file.strategy;
    let tokens = file.rendered_tokens;
    let saved = file.saved_tokens;

    buf.push_str(&format!("### File: {path_display}\n\n"));
    buf.push_str(&format!("*Strategy: {strategy} | Tokens: {tokens} (Saved: {saved})*\n\n"));

    // Get the content for the assigned strategy
    if let Some(strategy_data) = file.parsed.strategies_data.get(strategy) {
        let lang = language_fence(&file.parsed.language);
        buf.push_str(&format!("```{lang}\n"));
        buf.push_str(&strategy_data.content);
        if !strategy_data.content.ends_with('\n') {
            buf.push('\n');
        }
        buf.push_str("```\n\n");
    }
}

/// Get the markdown fence language identifier for a `Language`.
#[expect(clippy::missing_const_for_fn)]
fn language_fence(lang: &Language) -> &str {
    match lang {
        Language::Rust => "rust",
        Language::Python => "python",
        Language::TypeScript => "typescript",
        Language::Go => "go",
        Language::C => "c",
        Language::Generic(name) => name.as_str(),
    }
}

/// Apply anti-bloat rules to the rendered output:
/// 1. Compress consecutive blank lines to a single blank line
/// 2. Trim trailing whitespace from each line
fn apply_anti_bloat(text: &str) -> String {
    let mut result = String::with_capacity(text.len());
    let mut prev_blank = false;

    for line in text.lines() {
        let trimmed = line.trim_end();
        let is_blank = trimmed.is_empty();

        if is_blank && prev_blank {
            continue; // skip consecutive blank lines
        }

        result.push_str(trimmed);
        result.push('\n');
        prev_blank = is_blank;
    }

    // Ensure file ends with a single newline (no trailing blank lines)
    while result.ends_with("\n\n") {
        result.pop();
    }

    result
}

#[cfg(test)]
#[expect(clippy::unwrap_used, clippy::panic)]
mod tests {
    use std::path::PathBuf;

    use super::*;
    use crate::{
        config::OutputStrategy,
        ingestion::{GitContext, IngestionResult},
        parser::{ParsedFile, StrategyData},
        scheduler::{ScheduleResult, ScheduledFile},
    };

    fn make_scheduled_file(
        path: &str,
        lang: Language,
        strategy: OutputStrategy,
        content: &str,
        rendered_tokens: usize,
        saved_tokens: usize,
    ) -> ScheduledFile {
        let mut strategies_data = HashMap::new();
        strategies_data.insert(
            OutputStrategy::Full,
            StrategyData { content: content.to_string(), token_count: rendered_tokens },
        );
        strategies_data.insert(
            OutputStrategy::NoTests,
            StrategyData { content: content.to_string(), token_count: rendered_tokens },
        );
        strategies_data.insert(
            OutputStrategy::Summary,
            StrategyData { content: content.to_string(), token_count: rendered_tokens },
        );

        ScheduledFile {
            parsed: ParsedFile {
                path: PathBuf::from(path),
                language: lang,
                source: content.to_string(),
                strategies_data,
            },
            strategy,
            rendered_tokens,
            saved_tokens,
        }
    }

    fn make_ingestion(directory_tree: &str, git_context: Option<GitContext>) -> IngestionResult {
        IngestionResult { files: vec![], directory_tree: directory_tree.to_string(), git_context }
    }

    fn make_schedule(files: Vec<ScheduledFile>) -> ScheduleResult {
        let total_tokens = files.iter().map(|f| f.rendered_tokens).sum();
        let mut strategy_counts = HashMap::new();
        for f in &files {
            *strategy_counts.entry(f.strategy).or_insert(0) += 1;
        }
        ScheduleResult { total_tokens, raw_tokens: total_tokens, files, strategy_counts }
    }

    #[test]
    fn output_contains_repository_header() {
        let scheduled = make_schedule(vec![]);
        let ingestion = make_ingestion("", None);
        let output = render(&scheduled, &ingestion, "my-project", "A cool project");

        assert!(output.contains("# Repository: my-project"), "missing repository header");
        assert!(output.contains("> A cool project"), "missing description");
    }

    #[test]
    fn output_contains_directory_tree_section() {
        let tree = "my-project\n└── src\n    └── main.rs [Rust]";
        let file = make_scheduled_file(
            "src/main.rs",
            Language::Rust,
            OutputStrategy::Full,
            "fn main() {}",
            10,
            0,
        );
        let scheduled = make_schedule(vec![file]);
        let ingestion = make_ingestion(tree, None);
        let output = render(&scheduled, &ingestion, "my-project", "");

        assert!(output.contains("## Structure & Symbol Index"), "missing structure section");
        assert!(output.contains("### Directory Tree"), "missing directory tree header");
        assert!(output.contains("main.rs"), "missing main.rs in tree");
    }

    #[test]
    fn output_contains_source_files_section() {
        let file = make_scheduled_file(
            "src/main.rs",
            Language::Rust,
            OutputStrategy::Full,
            "fn main() {}",
            10,
            0,
        );
        let scheduled = make_schedule(vec![file]);
        let ingestion = make_ingestion("", None);
        let output = render(&scheduled, &ingestion, "test", "");

        assert!(output.contains("## Source Files"), "missing source files section");
        assert!(output.contains("### File: src/main.rs"), "missing file header");
    }

    #[test]
    fn each_source_file_has_strategy_annotation() {
        let file = make_scheduled_file(
            "src/lib.rs",
            Language::Rust,
            OutputStrategy::NoTests,
            "pub fn lib() {}",
            5,
            3,
        );
        let scheduled = make_schedule(vec![file]);
        let ingestion = make_ingestion("", None);
        let output = render(&scheduled, &ingestion, "test", "");

        assert!(output.contains("Strategy: NoTests"), "missing strategy annotation");
        assert!(output.contains("Tokens: 5"), "missing token count");
        assert!(output.contains("Saved: 3"), "missing saved count");
    }

    #[test]
    fn anti_bloat_compresses_blank_lines() {
        let content = "fn main() {\n\n\n\n    println!(\"hi\");\n}\n";
        let file = make_scheduled_file(
            "src/main.rs",
            Language::Rust,
            OutputStrategy::Full,
            content,
            10,
            0,
        );
        // Inject extra blank lines into the tree to trigger compression
        let scheduled = make_schedule(vec![file]);
        let ingestion = make_ingestion("project\n\n\n\n└── src", None);
        let output = render(&scheduled, &ingestion, "test", "");

        // Should not have 3+ consecutive newlines
        assert!(!output.contains("\n\n\n"), "consecutive blank lines not compressed: {output:?}");
    }

    #[test]
    fn anti_bloat_trims_trailing_whitespace() {
        let content = "fn main() {   \n    println!(\"hi\");   \n}\n";
        let file = make_scheduled_file(
            "src/main.rs",
            Language::Rust,
            OutputStrategy::Full,
            content,
            10,
            0,
        );
        let scheduled = make_schedule(vec![file]);
        let ingestion = make_ingestion("", None);
        let output = render(&scheduled, &ingestion, "test", "");

        for line in output.lines() {
            assert_eq!(line, line.trim_end(), "trailing whitespace found in line: {line:?}");
        }
    }

    #[test]
    fn git_context_section_when_enabled() {
        let git = GitContext {
            branch: "main".to_string(),
            latest_commit: "abc123 feat: add feature".to_string(),
            diff: Some("M src/main.rs".to_string()),
        };
        let scheduled = make_schedule(vec![]);
        let ingestion = make_ingestion("", Some(git));
        let output = render(&scheduled, &ingestion, "test", "");

        assert!(output.contains("### Git Context"), "missing git context section");
        assert!(output.contains("**Branch**: main"), "missing branch");
        assert!(output.contains("**Latest Commit**: abc123 feat: add feature"), "missing commit");
        assert!(output.contains("**Uncommitted Changes**: M src/main.rs"), "missing diff");
    }

    #[test]
    fn no_git_context_section_when_disabled() {
        let scheduled = make_schedule(vec![]);
        let ingestion = make_ingestion("", None);
        let output = render(&scheduled, &ingestion, "test", "");

        assert!(!output.contains("### Git Context"), "git context section should be absent");
        assert!(!output.contains("Branch"), "branch should be absent");
    }

    #[test]
    fn strategy_content_rendered_in_code_blocks() {
        let content = "pub fn hello() {\n    println!(\"world\");\n}\n";
        let file =
            make_scheduled_file("src/lib.rs", Language::Rust, OutputStrategy::Full, content, 10, 0);
        let scheduled = make_schedule(vec![file]);
        let ingestion = make_ingestion("", None);
        let output = render(&scheduled, &ingestion, "test", "");

        assert!(output.contains("```rust"), "missing rust code fence");
        assert!(output.contains("pub fn hello()"), "missing code content");
        assert!(output.contains("```"), "missing closing code fence");
    }

    #[test]
    fn token_counts_shown_in_file_headers() {
        let file = make_scheduled_file(
            "src/main.go",
            Language::Go,
            OutputStrategy::Summary,
            "func main()",
            3,
            7,
        );
        let scheduled = make_schedule(vec![file]);
        let ingestion = make_ingestion("", None);
        let output = render(&scheduled, &ingestion, "test", "");

        assert!(output.contains("Tokens: 3"), "missing rendered token count");
        assert!(output.contains("Saved: 7"), "missing saved token count");
        assert!(output.contains("Strategy: Summary"), "missing strategy");
    }

    #[test]
    fn directory_tree_annotated_with_strategy() {
        let tree = "project\n└── src\n    └── main.rs [Rust]";
        let file = make_scheduled_file(
            "src/main.rs",
            Language::Rust,
            OutputStrategy::NoTests,
            "fn main() {}",
            10,
            2,
        );
        let scheduled = make_schedule(vec![file]);
        let ingestion = make_ingestion(tree, None);
        let output = render(&scheduled, &ingestion, "test", "");

        // The tree should have the strategy annotation instead of language
        assert!(output.contains("[NoTests]"), "missing strategy annotation in tree");
    }

    #[test]
    fn multiple_files_all_rendered() {
        let files = vec![
            make_scheduled_file(
                "src/main.rs",
                Language::Rust,
                OutputStrategy::Full,
                "fn main() {}",
                10,
                0,
            ),
            make_scheduled_file(
                "src/lib.rs",
                Language::Rust,
                OutputStrategy::NoTests,
                "pub fn lib() {}",
                8,
                2,
            ),
            make_scheduled_file(
                "app.py",
                Language::Python,
                OutputStrategy::Summary,
                "def main(): ...",
                3,
                12,
            ),
        ];
        let scheduled = make_schedule(files);
        let ingestion = make_ingestion("", None);
        let output = render(&scheduled, &ingestion, "test", "");

        assert!(output.contains("### File: src/main.rs"), "missing main.rs");
        assert!(output.contains("### File: src/lib.rs"), "missing lib.rs");
        assert!(output.contains("### File: app.py"), "missing app.py");
        assert!(output.contains("Strategy: Full"), "missing Full strategy");
        assert!(output.contains("Strategy: NoTests"), "missing NoTests strategy");
        assert!(output.contains("Strategy: Summary"), "missing Summary strategy");
    }

    #[test]
    fn code_fence_has_correct_language() {
        let py_file = make_scheduled_file(
            "app.py",
            Language::Python,
            OutputStrategy::Full,
            "print('hi')",
            5,
            0,
        );
        let go_file = make_scheduled_file(
            "main.go",
            Language::Go,
            OutputStrategy::Full,
            "package main",
            5,
            0,
        );
        let ts_file = make_scheduled_file(
            "index.ts",
            Language::TypeScript,
            OutputStrategy::Full,
            "console.log('hi')",
            5,
            0,
        );

        let scheduled = make_schedule(vec![py_file, go_file, ts_file]);
        let ingestion = make_ingestion("", None);
        let output = render(&scheduled, &ingestion, "test", "");

        assert!(output.contains("```python"), "missing python fence");
        assert!(output.contains("```go"), "missing go fence");
        assert!(output.contains("```typescript"), "missing typescript fence");
    }

    #[test]
    fn description_blockquote_format() {
        let scheduled = make_schedule(vec![]);
        let ingestion = make_ingestion("", None);
        let output = render(&scheduled, &ingestion, "proj", "My description");

        assert!(output.contains("> My description"), "missing description blockquote");
        assert!(
            output.contains("> Note: This codebase has been optimized"),
            "missing optimization note"
        );
    }

    use proptest::prelude::*;

    fn arbitrary_content_strategy() -> impl Strategy<Value = String> {
        proptest::string::string_regex("[a-zA-Z0-9 \t\n(){};:,._=+\\-*/!@#$%^&|<>?~`]{0,300}")
            .unwrap()
    }

    proptest! {
        #[test]
        fn anti_bloat_no_consecutive_blank_lines(
            content in arbitrary_content_strategy(),
            num_files in 0_usize..5,
        ) {
            let files: Vec<ScheduledFile> = (0..num_files)
                .map(|i| {
                    make_scheduled_file(
                        &format!("src/file{i}.rs"),
                        Language::Rust,
                        OutputStrategy::Full,
                        &content,
                        10,
                        0,
                    )
                })
                .collect();
            let scheduled = make_schedule(files);
            let ingestion = make_ingestion("", None);
            let output = render(&scheduled, &ingestion, "test-project", "A description");
            prop_assert!(
                !output.contains("\n\n\n"),
                "output contains consecutive blank lines (3+ newlines)",
            );
        }
    }
}
