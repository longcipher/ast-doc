//! Phase 4: Output rendering.
//!
//! Assembles the final `llms.txt` markdown output from scheduled files,
//! directory tree, and git context.

pub mod llms_txt;

use crate::{config::AstDocConfig, ingestion::IngestionResult, scheduler::ScheduleResult};

/// Render the final `llms.txt` output.
///
/// # Errors
///
/// Returns an error if rendering fails.
pub fn render_llms_txt(
    scheduled: &ScheduleResult,
    ingestion: &IngestionResult,
    config: &AstDocConfig,
) -> eyre::Result<String> {
    let project_name = config
        .path
        .file_name()
        .map_or_else(|| "unknown".to_string(), |n| n.to_string_lossy().to_string());

    let output = llms_txt::render(scheduled, ingestion, &project_name, "");

    tracing::info!(
        files = scheduled.files.len(),
        tokens = scheduled.total_tokens,
        "rendering complete"
    );

    Ok(output)
}
