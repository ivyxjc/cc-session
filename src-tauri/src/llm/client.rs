//! Minimal OpenAI-compatible HTTP client.
//! Targets any endpoint speaking the `POST /chat/completions` shape.

use serde::{Deserialize, Serialize};
use std::time::Duration;

#[derive(Debug)]
pub enum LlmError {
    Http { status: u16, body: String },
    Transport(String),
    InvalidResponse(String),
}

impl std::fmt::Display for LlmError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Http { status, body } => write!(f, "HTTP {}: {}", status, body),
            Self::Transport(s) => write!(f, "transport error: {}", s),
            Self::InvalidResponse(s) => write!(f, "invalid response: {}", s),
        }
    }
}

impl std::error::Error for LlmError {}

#[derive(Debug, Clone)]
pub struct LlmConfig {
    pub base_url: String,
    pub api_key: String,
    pub model: String,
}

#[derive(Serialize)]
struct ChatRequest<'a> {
    model: &'a str,
    messages: Vec<ChatMessage<'a>>,
    max_tokens: u32,
    temperature: f32,
}

#[derive(Serialize)]
struct ChatMessage<'a> {
    role: &'a str,
    content: &'a str,
}

#[derive(Deserialize)]
struct ChatResponse {
    choices: Vec<ChatChoice>,
}

#[derive(Deserialize)]
struct ChatChoice {
    message: ChatChoiceMessage,
}

#[derive(Deserialize)]
struct ChatChoiceMessage {
    content: String,
}

pub struct LlmClient {
    cfg: LlmConfig,
    http: reqwest::Client,
}

impl LlmClient {
    pub fn new(cfg: LlmConfig) -> Self {
        let http = reqwest::Client::builder()
            .timeout(Duration::from_secs(60))
            .build()
            .expect("build reqwest client");
        Self { cfg, http }
    }

    fn endpoint(&self) -> String {
        format!("{}/chat/completions", self.cfg.base_url.trim_end_matches('/'))
    }

    /// Single completion call. Returns the raw `content` string from the first choice.
    pub async fn complete(&self, system: &str, user: &str, max_tokens: u32) -> Result<String, LlmError> {
        let body = ChatRequest {
            model: &self.cfg.model,
            messages: vec![
                ChatMessage { role: "system", content: system },
                ChatMessage { role: "user", content: user },
            ],
            max_tokens,
            temperature: 0.2,
        };
        let resp = self
            .http
            .post(self.endpoint())
            .bearer_auth(&self.cfg.api_key)
            .json(&body)
            .send()
            .await
            .map_err(|e| LlmError::Transport(e.to_string()))?;
        let status = resp.status();
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            return Err(LlmError::Http { status: status.as_u16(), body });
        }
        let parsed: ChatResponse = resp
            .json()
            .await
            .map_err(|e| LlmError::InvalidResponse(format!("response decode: {}", e)))?;
        let content = parsed
            .choices
            .into_iter()
            .next()
            .ok_or_else(|| LlmError::InvalidResponse("no choices".into()))?
            .message
            .content;
        Ok(content)
    }

    /// Lightweight ping for the "Test connection" button.
    pub async fn ping(&self) -> Result<String, LlmError> {
        // 16 is Azure's minimum for `max_output_tokens`; safe across all providers.
        let content = self.complete("You are a test.", "Reply with the single word OK.", 16).await?;
        Ok(content)
    }
}

/// Strip common JSON-output wrappers (markdown code fences) and parse.
pub fn parse_json_payload<T: for<'de> Deserialize<'de>>(raw: &str) -> Result<T, LlmError> {
    let trimmed = raw.trim();
    // Strip ```json ... ``` or ``` ... ``` fences if present
    let stripped = if trimmed.starts_with("```") {
        let after_open = trimmed.trim_start_matches("```json").trim_start_matches("```").trim_start_matches('\n');
        after_open
            .rsplit_once("```")
            .map(|(body, _)| body)
            .unwrap_or(after_open)
            .trim()
    } else {
        trimmed
    };
    serde_json::from_str(stripped)
        .map_err(|e| LlmError::InvalidResponse(format!("json parse: {} (raw: {})", e, stripped.chars().take(200).collect::<String>())))
}
