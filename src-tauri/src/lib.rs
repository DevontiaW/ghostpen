use serde::{Deserialize, Serialize};
use harper_core::linting::{LintGroup, Linter};
use harper_core::spell::FstDictionary;
use harper_core::{Document, Dialect};
use std::io::Write;
use std::sync::Arc;

mod audit;
mod llm;

#[derive(Serialize, Clone)]
pub struct GrammarIssue {
    pub start: usize,
    pub end: usize,
    pub message: String,
    pub suggestions: Vec<String>,
    pub severity: String,
}

#[derive(Serialize)]
pub struct CheckResult {
    pub issues: Vec<GrammarIssue>,
    pub stats: TextStats,
}

#[derive(Serialize)]
pub struct TextStats {
    pub word_count: usize,
    pub sentence_count: usize,
    pub issue_count: usize,
}

#[derive(Deserialize)]
pub struct RewriteRequest {
    pub text: String,
    pub mode: String,
}

#[derive(Serialize)]
pub struct RewriteResult {
    pub rewritten: String,
    pub explanation: String,
}

#[derive(Serialize)]
pub struct LlmStatus {
    pub available: bool,
    pub provider: String,
    pub model: String,
}

/// Check text for grammar issues using Harper (instant, local, no network)
#[tauri::command]
fn check_grammar(text: &str) -> CheckResult {
    let start_time = std::time::Instant::now();

    let dict = FstDictionary::curated();
    let document = Document::new_plain_english(text, &dict);
    let mut linter = LintGroup::new_curated(Arc::clone(&dict), Dialect::American);
    let lints = linter.lint(&document);

    let issues: Vec<GrammarIssue> = lints
        .iter()
        .map(|lint| {
            let start = lint.span.start;
            let end = lint.span.end;

            // Pre-expand suggestions so the frontend can treat all as simple replacements
            let original_span = if end <= text.len() {
                &text[start..end]
            } else {
                ""
            };
            let suggestions: Vec<String> = lint
                .suggestions
                .iter()
                .filter_map(|s| match s {
                    harper_core::linting::Suggestion::ReplaceWith(chars) => {
                        Some(chars.iter().collect::<String>())
                    }
                    harper_core::linting::Suggestion::InsertAfter(chars) => {
                        // InsertAfter means keep original + append these chars
                        let insert: String = chars.iter().collect();
                        Some(format!("{}{}", original_span, insert))
                    }
                    harper_core::linting::Suggestion::Remove => Some(String::new()),
                })
                .collect();

            GrammarIssue {
                start,
                end,
                message: lint.message.clone(),
                suggestions,
                severity: format!("{:?}", lint.lint_kind),
            }
        })
        .collect();

    let word_count = text.split_whitespace().count();
    let sentence_count = text.chars()
        .filter(|c| *c == '.' || *c == '!' || *c == '?')
        .count()
        .max(1);

    let duration_ms = start_time.elapsed().as_millis();
    let issue_count = issues.len();

    audit::log_event("grammar_check", serde_json::json!({
        "word_count": word_count,
        "issue_count": issue_count,
        "duration_ms": duration_ms,
    }));

    CheckResult {
        stats: TextStats {
            word_count,
            sentence_count,
            issue_count,
        },
        issues,
    }
}

/// Rewrite text using local LLM (Ollama or LM Studio)
#[tauri::command]
async fn rewrite_text(request: RewriteRequest) -> Result<RewriteResult, String> {
    let text_length = request.text.len();
    let mode = request.mode.clone();

    let result = llm::rewrite(&request.text, &request.mode)
        .await
        .map_err(|e| e.to_string());

    let (success, provider) = match &result {
        Ok(_) => (true, "detected".to_string()),
        Err(e) => (false, e.clone()),
    };

    audit::log_event("rewrite", serde_json::json!({
        "mode": mode,
        "text_length": text_length,
        "success": success,
        "provider": provider,
    }));

    result
}

/// Check if a local LLM server is running
#[tauri::command]
async fn check_llm_status() -> Result<LlmStatus, String> {
    let result = llm::check_status().await.map_err(|e| e.to_string());

    if let Ok(ref status) = result {
        audit::log_event("llm_status_check", serde_json::json!({
            "available": status.available,
            "provider": status.provider,
        }));
    }

    result
}

/// Launch LM Studio in the background
#[tauri::command]
fn launch_llm() -> Result<String, String> {
    let result = llm::launch_lm_studio();

    match &result {
        Ok(msg) => audit::log_event("llm_launch", serde_json::json!({
            "success": true,
            "path_or_error": msg,
        })),
        Err(e) => audit::log_event("llm_launch", serde_json::json!({
            "success": false,
            "path_or_error": e,
        })),
    }

    result
}

#[derive(Deserialize)]
pub struct FeedbackRequest {
    pub rating: String,
    pub original_text: String,
    pub rewritten_text: String,
    pub mode: String,
}

/// Save user feedback on a rewrite to ~/.ghostpen/feedback.jsonl
#[tauri::command]
fn save_feedback(feedback: FeedbackRequest) -> Result<String, String> {
    let ghostpen_dir = dirs::home_dir()
        .ok_or_else(|| "Could not determine home directory".to_string())?
        .join(".ghostpen");

    std::fs::create_dir_all(&ghostpen_dir)
        .map_err(|e| format!("Failed to create .ghostpen directory: {}", e))?;

    let feedback_path = ghostpen_dir.join("feedback.jsonl");

    let entry = serde_json::json!({
        "timestamp": chrono::Utc::now().to_rfc3339(),
        "rating": feedback.rating,
        "original_text": feedback.original_text,
        "rewritten_text": feedback.rewritten_text,
        "mode": feedback.mode,
    });

    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&feedback_path)
        .map_err(|e| format!("Failed to open feedback file: {}", e))?;

    writeln!(file, "{}", entry.to_string())
        .map_err(|e| format!("Failed to write feedback: {}", e))?;

    audit::log_event("feedback", serde_json::json!({
        "rating": feedback.rating,
        "mode": feedback.mode,
    }));

    Ok("ok".to_string())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            check_grammar,
            rewrite_text,
            check_llm_status,
            launch_llm,
            save_feedback,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
