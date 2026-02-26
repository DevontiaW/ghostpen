use serde::{Deserialize, Serialize};
use harper_core::linting::{LintGroup, Linter};
use harper_core::spell::FstDictionary;
use harper_core::{Document, Dialect};
use std::sync::Arc;

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
    let dict = FstDictionary::curated();
    let document = Document::new_plain_english(text, &dict);
    let mut linter = LintGroup::new_curated(Arc::clone(&dict), Dialect::American);
    let lints = linter.lint(&document);

    let issues: Vec<GrammarIssue> = lints
        .iter()
        .map(|lint| {
            let start = lint.span.start;
            let end = lint.span.end;

            // Serialize suggestions as their debug representation for now
            // TODO: extract proper replacement text once we understand the full API
            let suggestions: Vec<String> = lint
                .suggestions
                .iter()
                .filter_map(|s| match s {
                    harper_core::linting::Suggestion::ReplaceWith(chars) => {
                        Some(chars.iter().collect::<String>())
                    }
                    harper_core::linting::Suggestion::InsertAfter(chars) => {
                        Some(chars.iter().collect::<String>())
                    }
                    harper_core::linting::Suggestion::Remove => None,
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

    CheckResult {
        stats: TextStats {
            word_count,
            sentence_count,
            issue_count: issues.len(),
        },
        issues,
    }
}

/// Rewrite text using local LLM (Ollama or LM Studio)
#[tauri::command]
async fn rewrite_text(request: RewriteRequest) -> Result<RewriteResult, String> {
    llm::rewrite(&request.text, &request.mode)
        .await
        .map_err(|e| e.to_string())
}

/// Check if a local LLM server is running
#[tauri::command]
async fn check_llm_status() -> Result<LlmStatus, String> {
    llm::check_status().await.map_err(|e| e.to_string())
}

/// Launch LM Studio in the background
#[tauri::command]
fn launch_llm() -> Result<String, String> {
    llm::launch_lm_studio()
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
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
