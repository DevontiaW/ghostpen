use serde::{Deserialize, Serialize};
use crate::{RewriteResult, LlmStatus};

// Both Ollama and LM Studio serve OpenAI-compatible API on these ports
// Use 127.0.0.1 instead of localhost — Windows can resolve localhost to IPv6 ::1
// while LM Studio / Ollama only bind to IPv4
const LMSTUDIO_URL: &str = "http://127.0.0.1:1234";
const OLLAMA_LOCAL_URL: &str = "http://127.0.0.1:11434";

// Default models (user can change later)
const OLLAMA_MODEL: &str = "qwen2.5:3b";
const LMSTUDIO_MODEL: &str = "default"; // LM Studio uses whatever model is loaded

#[derive(Serialize)]
struct ChatRequest {
    model: String,
    messages: Vec<ChatMessage>,
    stream: bool,
    temperature: f32,
}

#[derive(Serialize)]
struct ChatMessage {
    role: String,
    content: String,
}

#[derive(Deserialize)]
struct ChatResponse {
    choices: Vec<ChatChoice>,
}

#[derive(Deserialize)]
struct ChatChoice {
    message: ChatResponseMessage,
}

#[derive(Deserialize)]
struct ChatResponseMessage {
    content: String,
}

enum Provider {
    Ollama,
    LmStudio,
}

async fn detect_provider() -> Result<(Provider, String), Box<dyn std::error::Error + Send + Sync>> {
    let client = reqwest::Client::new();
    let timeout = std::time::Duration::from_secs(2);

    // Try LM Studio first (most common for desktop users)
    if let Ok(resp) = client
        .get(format!("{}/v1/models", LMSTUDIO_URL))
        .timeout(timeout)
        .send()
        .await
    {
        if resp.status().is_success() {
            return Ok((Provider::LmStudio, LMSTUDIO_URL.to_string()));
        }
    }

    // Try local Ollama
    if let Ok(resp) = client
        .get(OLLAMA_LOCAL_URL)
        .timeout(timeout)
        .send()
        .await
    {
        if resp.status().is_success() {
            return Ok((Provider::Ollama, OLLAMA_LOCAL_URL.to_string()));
        }
    }

    Err("No LLM server found. Install Ollama or LM Studio.".into())
}

/// Attempt to launch LM Studio in the background
pub fn launch_lm_studio() -> Result<String, String> {
    // Try common LM Studio paths on Windows
    let paths = [
        dirs::home_dir().map(|h| h.join(".lmstudio/bin/lms.exe")),
        dirs::data_local_dir().map(|d| d.join("Programs/LM Studio/LM Studio.exe")),
    ];

    for maybe_path in &paths {
        if let Some(path) = maybe_path {
            if path.exists() {
                // lms CLI: start the server in background
                if path.to_string_lossy().contains("lms") {
                    match std::process::Command::new(path)
                        .args(["server", "start"])
                        .spawn()
                    {
                        Ok(_) => return Ok(format!("LM Studio server starting via {}", path.display())),
                        Err(e) => return Err(format!("Failed to launch: {}", e)),
                    }
                }
                // GUI path: launch the app
                match std::process::Command::new(path).spawn() {
                    Ok(_) => return Ok(format!("LM Studio launching from {}", path.display())),
                    Err(e) => return Err(format!("Failed to launch: {}", e)),
                }
            }
        }
    }

    Err("LM Studio not found. Install from https://lmstudio.ai".to_string())
}

pub async fn check_status() -> Result<LlmStatus, Box<dyn std::error::Error + Send + Sync>> {
    match detect_provider().await {
        Ok((Provider::Ollama, _)) => Ok(LlmStatus {
            available: true,
            provider: "Ollama".to_string(),
            model: OLLAMA_MODEL.to_string(),
        }),
        Ok((Provider::LmStudio, _)) => Ok(LlmStatus {
            available: true,
            provider: "LM Studio".to_string(),
            model: LMSTUDIO_MODEL.to_string(),
        }),
        Err(_) => Ok(LlmStatus {
            available: false,
            provider: "none".to_string(),
            model: String::new(),
        }),
    }
}

pub async fn rewrite(text: &str, mode: &str) -> Result<RewriteResult, Box<dyn std::error::Error + Send + Sync>> {
    let (provider, base_url) = detect_provider().await?;

    let model = match provider {
        Provider::Ollama => OLLAMA_MODEL.to_string(),
        Provider::LmStudio => LMSTUDIO_MODEL.to_string(),
    };

    let system_prompt = "You are a writing assistant. You help improve text while preserving the writer's voice. Always explain WHY you made changes so the writer learns. Be concise.";
    let user_prompt = build_prompt(text, mode);

    // Both Ollama and LM Studio support OpenAI-compatible /v1/chat/completions
    let api_url = match provider {
        Provider::Ollama => format!("{}/v1/chat/completions", base_url),
        Provider::LmStudio => format!("{}/v1/chat/completions", base_url),
    };

    let client = reqwest::Client::new();
    let resp = client
        .post(&api_url)
        .json(&ChatRequest {
            model,
            messages: vec![
                ChatMessage {
                    role: "system".to_string(),
                    content: system_prompt.to_string(),
                },
                ChatMessage {
                    role: "user".to_string(),
                    content: user_prompt,
                },
            ],
            stream: false,
            temperature: 0.3,
        })
        .timeout(std::time::Duration::from_secs(180))
        .send()
        .await?
        .json::<ChatResponse>()
        .await?;

    let full = resp
        .choices
        .first()
        .map(|c| c.message.content.trim().to_string())
        .unwrap_or_default();

    // Parse response — try to split rewrite from explanation
    let (rewritten, explanation) = parse_response(&full);

    Ok(RewriteResult {
        rewritten,
        explanation,
    })
}

fn parse_response(full: &str) -> (String, String) {
    // Try various delimiter patterns
    for delimiter in &["EXPLANATION:", "**Explanation:**", "**Why:**", "---", "\n\n**Changes"] {
        if let Some(idx) = full.find(delimiter) {
            let rewrite = full[..idx].trim();
            let explain = full[idx + delimiter.len()..].trim();
            // Strip "REWRITE:" prefix if present
            let rewrite = rewrite
                .strip_prefix("REWRITE:")
                .or_else(|| rewrite.strip_prefix("**Rewrite:**"))
                .unwrap_or(rewrite)
                .trim();
            return (rewrite.to_string(), explain.to_string());
        }
    }

    // No delimiter found — entire response is the rewrite
    let cleaned = full
        .strip_prefix("REWRITE:")
        .unwrap_or(full)
        .trim();
    (cleaned.to_string(), String::new())
}

fn build_prompt(text: &str, mode: &str) -> String {
    match mode {
        "clarity" => format!(
            "Rewrite this text for maximum clarity. Keep the meaning identical.\n\nFirst, provide the rewritten text. Then write EXPLANATION: followed by what you changed and why the writer should care (teach them).\n\nText: {}", text
        ),
        "concise" => format!(
            "Make this text more concise. Cut unnecessary words without losing meaning.\n\nFirst, provide the rewritten text. Then write EXPLANATION: followed by what you cut and why it was unnecessary (teach the writer to self-edit).\n\nText: {}", text
        ),
        "formal" => format!(
            "Rewrite in a more formal, professional tone.\n\nFirst, provide the rewritten text. Then write EXPLANATION: followed by what tone shifts you made and when formal tone matters.\n\nText: {}", text
        ),
        "casual" => format!(
            "Rewrite in a more casual, conversational tone.\n\nFirst, provide the rewritten text. Then write EXPLANATION: followed by what you changed to make it more natural.\n\nText: {}", text
        ),
        "explain" => format!(
            "Analyze this text as a writing coach. Identify grammar issues, unclear phrasing, and style problems. For each issue, explain WHAT is wrong and WHY it matters — teach the writer, don't just flag.\n\nText: {}", text
        ),
        _ => format!(
            "Improve this text for clarity and correctness.\n\nFirst, provide the improved text. Then write EXPLANATION: followed by a brief teaching note.\n\nText: {}", text
        ),
    }
}
