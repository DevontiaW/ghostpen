use serde::{Deserialize, Serialize};
use harper_core::linting::{LintGroup, Linter};
use harper_core::spell::FstDictionary;
use harper_core::{Document, Dialect};
use std::io::Write;
use std::sync::{Arc, OnceLock};
use regex::Regex;

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

fn double_space_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"  +").unwrap())
}

fn repeated_punct_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    // Rust regex crate doesn't support backreferences, so match each char type separately
    RE.get_or_init(|| Regex::new(r"!{2,}|\?{2,}|\.{2,}").unwrap())
}

fn sentence_line_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r#"(?m)^[A-Z][^\n]{10,}[a-z0-9)\]"'\-*`]$"#).unwrap())
}

/// Check for punctuation issues that Harper doesn't catch
fn check_punctuation(text: &str) -> Vec<GrammarIssue> {
    let mut issues = Vec::new();

    // Double spaces
    for m in double_space_re().find_iter(text) {
        let start_char = text[..m.start()].chars().count();
        let end_char = text[..m.end()].chars().count();
        issues.push(GrammarIssue {
            start: start_char,
            end: end_char,
            message: "Multiple spaces found. Use a single space.".to_string(),
            suggestions: vec![" ".to_string()],
            severity: "Style".to_string(),
        });
    }

    // Repeated punctuation (!!, ??, ..)
    for m in repeated_punct_re().find_iter(text) {
        let matched = m.as_str();
        // Allow "..." (ellipsis) — only flag if not exactly 3 dots
        if matched.starts_with('.') && matched.len() == 3 {
            continue;
        }
        let start_char = text[..m.start()].chars().count();
        let end_char = text[..m.end()].chars().count();
        let single = matched.chars().next().unwrap().to_string();
        issues.push(GrammarIssue {
            start: start_char,
            end: end_char,
            message: format!("Repeated punctuation '{}'. Use a single character.", matched),
            suggestions: vec![single],
            severity: "Style".to_string(),
        });
    }

    // Sentence without ending punctuation — lines that look like sentences
    // (start with capital, have 3+ words, don't end with .!?:;)
    // Widened char class includes hyphens, asterisks, backticks
    for m in sentence_line_re().find_iter(text) {
        let line = m.as_str();
        let word_count = line.split_whitespace().count();
        if word_count >= 3 {
            // Point to the end of the line
            let end_char = text[..m.end()].chars().count();
            issues.push(GrammarIssue {
                start: end_char - 1,
                end: end_char,
                message: "Sentence may be missing ending punctuation.".to_string(),
                suggestions: vec![
                    format!("{}.", &line[line.len()-1..]),
                ],
                severity: "Style".to_string(),
            });
        }
    }

    issues
}

/// Read custom dictionary words from ~/.ghostpen/dictionary.txt
fn load_dictionary() -> Vec<String> {
    let Some(home) = dirs::home_dir() else { return vec![] };
    let dict_path = home.join(".ghostpen").join("dictionary.txt");
    match std::fs::read_to_string(&dict_path) {
        Ok(content) => content
            .lines()
            .map(|l| l.trim().to_lowercase())
            .filter(|l| !l.is_empty())
            .collect(),
        Err(_) => vec![],
    }
}

/// Add a word to the custom dictionary
#[tauri::command]
fn add_to_dictionary(word: String) -> Result<String, String> {
    let ghostpen_dir = dirs::home_dir()
        .ok_or_else(|| "Could not determine home directory".to_string())?
        .join(".ghostpen");

    std::fs::create_dir_all(&ghostpen_dir)
        .map_err(|e| format!("Failed to create .ghostpen directory: {}", e))?;

    let dict_path = ghostpen_dir.join("dictionary.txt");

    // Check if word already exists
    let existing = std::fs::read_to_string(&dict_path).unwrap_or_default();
    let lower = word.trim().to_lowercase();
    if existing.lines().any(|l| l.trim().to_lowercase() == lower) {
        return Ok("already exists".to_string());
    }

    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&dict_path)
        .map_err(|e| format!("Failed to open dictionary: {}", e))?;

    writeln!(file, "{}", word.trim())
        .map_err(|e| format!("Failed to write to dictionary: {}", e))?;

    Ok("ok".to_string())
}

/// Check text for grammar issues using Harper (instant, local, no network)
#[tauri::command]
fn check_grammar(text: &str) -> CheckResult {
    let start_time = std::time::Instant::now();

    let dict = FstDictionary::curated();
    let document = Document::new_plain_english(text, &dict);
    let mut linter = LintGroup::new_curated(Arc::clone(&dict), Dialect::American);
    let lints = linter.lint(&document);

    let mut issues: Vec<GrammarIssue> = lints
        .iter()
        .map(|lint| {
            let start = lint.span.start;
            let end = lint.span.end;

            // Harper returns byte offsets, but JS/CM6 expects character indices.
            // For multi-byte chars (emoji, accented letters, smart quotes), these differ.
            let start_char = text[..start.min(text.len())].chars().count();
            let end_char = text[..end.min(text.len())].chars().count();

            // Pre-expand suggestions so the frontend can treat all as simple replacements
            // Note: Rust string slicing uses byte offsets, so keep start/end for slicing.
            // Use .get() to avoid panicking if offsets land mid-character.
            let original_span = text.get(start..end).unwrap_or("");
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
                start: start_char,
                end: end_char,
                message: lint.message.clone(),
                suggestions,
                severity: format!("{:?}", lint.lint_kind),
            }
        })
        .collect();

    // Merge punctuation issues that Harper doesn't catch
    let mut punctuation_issues = check_punctuation(text);
    issues.append(&mut punctuation_issues);

    // Filter out issues for words in the custom dictionary
    let dictionary = load_dictionary();
    if !dictionary.is_empty() {
        issues.retain(|issue| {
            // Convert char offsets back to byte offsets for slicing
            let byte_start: usize = text.chars().take(issue.start).map(|c| c.len_utf8()).sum();
            let byte_end: usize = text.chars().take(issue.end).map(|c| c.len_utf8()).sum();
            let word = text.get(byte_start..byte_end)
                .unwrap_or("").trim().to_lowercase();
            !dictionary.contains(&word)
        });
    }

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
/// When called via rewrite_text_stream, emits "rewrite-stream" events with progressive text
#[tauri::command]
async fn rewrite_text(request: RewriteRequest) -> Result<RewriteResult, String> {
    let text_length = request.text.len();
    let mode = request.mode.clone();

    let result = llm::rewrite(&request.text, &request.mode, None)
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

/// Streaming rewrite — emits "rewrite-stream" events as tokens arrive
#[tauri::command]
async fn rewrite_text_stream(app: tauri::AppHandle, request: RewriteRequest) -> Result<RewriteResult, String> {
    let text_length = request.text.len();
    let mode = request.mode.clone();

    let result = llm::rewrite(&request.text, &request.mode, Some(&app))
        .await
        .map_err(|e| e.to_string());

    let (success, provider) = match &result {
        Ok(_) => (true, "detected".to_string()),
        Err(e) => (false, e.clone()),
    };

    audit::log_event("rewrite_stream", serde_json::json!({
        "mode": mode,
        "text_length": text_length,
        "success": success,
        "provider": provider,
    }));

    result
}

/// Cancel an in-flight rewrite request
#[tauri::command]
fn cancel_rewrite() {
    llm::request_cancel();
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
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .invoke_handler(tauri::generate_handler![
            check_grammar,
            add_to_dictionary,
            rewrite_text,
            rewrite_text_stream,
            cancel_rewrite,
            check_llm_status,
            launch_llm,
            save_feedback,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn punctuation_missing_period() {
        // The regex requires: starts with capital, 10+ chars, ends with [a-z0-9)\]"'\-*`] at end of line
        let issues = check_punctuation("This is a complete sentence without ending punctuation\nNext line.");
        assert!(issues.iter().any(|i| i.message.contains("missing ending punctuation")));
    }

    #[test]
    fn punctuation_has_period() {
        let issues = check_punctuation("This sentence has a period.");
        assert!(!issues.iter().any(|i| i.message.contains("missing ending punctuation")));
    }

    #[test]
    fn punctuation_question_mark() {
        let issues = check_punctuation("Is this a question?");
        assert!(!issues.iter().any(|i| i.message.contains("missing ending punctuation")));
    }

    #[test]
    fn punctuation_exclamation() {
        let issues = check_punctuation("What an exclamation!");
        assert!(!issues.iter().any(|i| i.message.contains("missing ending punctuation")));
    }

    #[test]
    fn punctuation_double_spaces() {
        let issues = check_punctuation("Two  spaces here.");
        assert!(issues.iter().any(|i| i.message.contains("Multiple spaces")));
    }

    #[test]
    fn punctuation_widened_chars() {
        // Lines ending in hyphens, asterisks, backticks should be caught
        let issues = check_punctuation("This is a line ending with a hyphen-\nNext.");
        // The hyphen at end of line should trigger missing punctuation
        assert!(issues.iter().any(|i| i.message.contains("missing ending punctuation")));
    }
}
