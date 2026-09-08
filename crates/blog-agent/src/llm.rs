use anyhow::{Context, Result};
use futures::stream::{Stream, StreamExt};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::pin::Pin;

#[derive(Clone)]
pub struct LlmConfig {
    pub model: String,
    pub api_key: String,
    pub endpoint: String,
    pub provider: LlmProvider,
}

#[derive(Clone, Debug, PartialEq)]
pub enum LlmProvider {
    OpenAI,
    Anthropic,
}

impl LlmConfig {
    pub fn from_env() -> Option<Self> {
        let model = std::env::var("LLM_MODEL").ok()?;
        let api_key = std::env::var("LLM_API_KEY").ok()?;
        let endpoint = std::env::var("LLM_ENDPOINT")
            .unwrap_or_else(|_| "https://api.openai.com/v1/chat/completions".to_string());

        let provider = if endpoint.contains("anthropic.com") {
            LlmProvider::Anthropic
        } else {
            LlmProvider::OpenAI
        };

        Some(Self {
            model,
            api_key,
            endpoint,
            provider,
        })
    }
}

pub struct LlmClient {
    config: LlmConfig,
    client: Client,
}

impl LlmClient {
    pub fn new(config: LlmConfig) -> Result<Self> {
        let client = Client::builder()
            .timeout(std::time::Duration::from_secs(60))
            .build()
            .context("Failed to create HTTP client")?;

        Ok(Self { config, client })
    }

    pub fn stream_completion(
        &self,
        system: String,
        user: String,
    ) -> Pin<Box<dyn Stream<Item = Result<String>> + Send>> {
        let config = self.config.clone();
        let client = self.client.clone();

        Box::pin(async_stream::stream! {
            let mut request_builder = client.post(&config.endpoint);

            match config.provider {
                LlmProvider::OpenAI => {
                    let request = OpenAIRequest {
                        model: config.model.clone(),
                        messages: vec![
                            Message {
                                role: "system".to_string(),
                                content: system,
                            },
                            Message {
                                role: "user".to_string(),
                                content: user,
                            },
                        ],
                        stream: true,
                        temperature: Some(0.7),
                        max_tokens: Some(2000),
                    };
                    request_builder = request_builder
                        .header("Authorization", format!("Bearer {}", config.api_key))
                        .json(&request);
                }
                LlmProvider::Anthropic => {
                    let request = AnthropicRequest {
                        model: config.model.clone(),
                        messages: vec![
                            Message {
                                role: "user".to_string(),
                                content: user,
                            },
                        ],
                        system: Some(system),
                        stream: true,
                        temperature: Some(0.7),
                        max_tokens: 2000,
                    };
                    request_builder = request_builder
                        .header("x-api-key", &config.api_key)
                        .header("anthropic-version", "2023-06-01")
                        .json(&request);
                }
            }

            let response = match request_builder
                .header("Content-Type", "application/json")
                .send()
                .await
            {
                Ok(resp) => resp,
                Err(e) => {
                    yield Err(anyhow::anyhow!("Request failed: {}", e));
                    return;
                }
            };

            if !response.status().is_success() {
                let status = response.status();
                let text = response.text().await.unwrap_or_default();
                yield Err(anyhow::anyhow!("LLM API error {}: {}", status, text));
                return;
            }

            let mut stream = response.bytes_stream();

            while let Some(chunk) = stream.next().await {
                match chunk {
                    Ok(bytes) => {
                        let text = String::from_utf8_lossy(&bytes);
                        for line in text.lines() {
                            if let Some(data) = line.strip_prefix("data: ") {
                                if data == "[DONE]" {
                                    return;
                                }

                                match config.provider {
                                    LlmProvider::OpenAI => {
                                        if let Ok(chunk) = serde_json::from_str::<OpenAIStreamChunk>(data) {
                                            if let Some(delta) = chunk.choices.first() {
                                                if let Some(content) = &delta.delta.content {
                                                    yield Ok(content.clone());
                                                }
                                            }
                                        }
                                    }
                                    LlmProvider::Anthropic => {
                                        if let Ok(event) = serde_json::from_str::<AnthropicStreamEvent>(data) {
                                            if event.event_type == "content_block_delta" {
                                                if let Some(delta) = event.delta {
                                                    if let Some(text) = delta.text {
                                                        yield Ok(text);
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                    Err(e) => {
                        yield Err(anyhow::anyhow!("Stream error: {}", e));
                        return;
                    }
                }
            }
        })
    }
}

#[derive(Serialize)]
struct OpenAIRequest {
    model: String,
    messages: Vec<Message>,
    stream: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_tokens: Option<u32>,
}

#[derive(Serialize)]
struct AnthropicRequest {
    model: String,
    messages: Vec<Message>,
    #[serde(skip_serializing_if = "Option::is_none")]
    system: Option<String>,
    stream: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
    max_tokens: u32,
}

#[derive(Serialize, Deserialize)]
struct Message {
    role: String,
    content: String,
}

#[derive(Deserialize)]
struct OpenAIStreamChunk {
    choices: Vec<Choice>,
}

#[derive(Deserialize)]
struct Choice {
    delta: Delta,
}

#[derive(Deserialize)]
struct Delta {
    content: Option<String>,
}

#[derive(Deserialize)]
struct AnthropicStreamEvent {
    #[serde(rename = "type")]
    event_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    delta: Option<AnthropicDelta>,
}

#[derive(Deserialize)]
struct AnthropicDelta {
    #[serde(skip_serializing_if = "Option::is_none")]
    text: Option<String>,
}
