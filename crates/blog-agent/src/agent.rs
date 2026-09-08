use anyhow::Result;
use axum::response::sse::Event;
use futures::stream::{Stream, StreamExt};
use serde::{Deserialize, Serialize};
use std::convert::Infallible;
use std::path::PathBuf;

use crate::llm::{LlmClient, LlmConfig};
use crate::retrieval::{Retriever, SearchResult};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatRequest {
    #[serde(default)]
    pub article_id: Option<String>,
    #[serde(default)]
    pub anchor: Option<String>,
    #[serde(default)]
    pub figure_id: Option<String>,
    #[serde(default)]
    pub selection: Option<String>,
    pub question: String,
    #[serde(default)]
    pub mode: AgentMode,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AgentMode {
    #[default]
    Reader,
    Author,
}

impl AgentMode {
    pub fn capabilities(&self) -> &'static [Capability] {
        match self {
            AgentMode::Reader => &[
                Capability::Retrieve,
                Capability::Explain,
                Capability::Summarize,
                Capability::CrossLink,
            ],
            AgentMode::Author => &[
                Capability::Retrieve,
                Capability::Explain,
                Capability::Summarize,
                Capability::CrossLink,
                Capability::ProposePatch,
                Capability::CheckCitations,
                Capability::TriggerBuild,
            ],
        }
    }

    pub fn allows(&self, cap: Capability) -> bool {
        self.capabilities().contains(&cap)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Capability {
    Retrieve,
    Explain,
    Summarize,
    CrossLink,
    ProposePatch,
    CheckCitations,
    TriggerBuild,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Source {
    pub article_id: String,
    pub section_anchor: String,
    pub section_title: String,
}

impl From<&SearchResult> for Source {
    fn from(r: &SearchResult) -> Self {
        Source {
            article_id: r.article_id.clone(),
            section_anchor: r.section_anchor.clone(),
            section_title: r.section_title.clone(),
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum AgentError {
    #[error("LLM not configured. Set LLM_MODEL and LLM_API_KEY environment variables.")]
    NotConfigured,
}

pub struct AgentCore {
    retriever: Retriever,
    llm: Option<LlmClient>,
}

impl AgentCore {
    pub fn new(index_dir: PathBuf, config: Option<LlmConfig>) -> Result<Self> {
        let retriever = Retriever::open(index_dir)?;
        let llm = config.map(LlmClient::new).transpose()?;
        Ok(Self { retriever, llm })
    }

    pub async fn chat(
        &self,
        request: &ChatRequest,
    ) -> Result<impl Stream<Item = Result<Event, Infallible>>, AgentError> {
        let llm = self.llm.as_ref().ok_or(AgentError::NotConfigured)?;

        let local = self.assemble_local_context(request);

        tracing::debug!("Searching for: {}", request.question);
        let results = self
            .retriever
            .search(&request.question, 5)
            .unwrap_or_else(|e| {
                tracing::error!("Search failed: {}", e);
                Vec::new()
            });
        tracing::debug!("Found {} results", results.len());

        let sources: Vec<Source> = results.iter().map(Source::from).collect();
        let prompt = build_prompt(request, &local, &results);

        let token_stream = llm.stream_completion(system_prompt(request.mode), prompt);

        let sources_json = serde_json::to_string(&serde_json::json!({
            "type": "sources",
            "sources": sources,
        }))
        .unwrap_or_else(|_| r#"{"type":"sources","sources":[]}"#.into());

        let head = futures::stream::once(async move { Ok(Event::default().data(sources_json)) });

        let body = token_stream.map(|chunk| {
            let payload = match chunk {
                Ok(text) => serde_json::json!({ "type": "token", "text": text }),
                Err(e) => serde_json::json!({ "type": "error", "error": e.to_string() }),
            };
            let data = serde_json::to_string(&payload)
                .unwrap_or_else(|_| r#"{"type":"error","error":"serialization failed"}"#.into());
            Ok(Event::default().data(data))
        });

        let tail = futures::stream::once(async { Ok(Event::default().data(r#"{"type":"done"}"#)) });

        Ok(head.chain(body).chain(tail))
    }

    fn assemble_local_context(&self, request: &ChatRequest) -> String {
        let mut parts = Vec::new();

        if let Some(article_id) = &request.article_id {
            parts.push(format!("Current article: {}", article_id));

            if let Some(anchor) = &request.anchor {
                match self.retriever.section_text(article_id, anchor) {
                    Ok(Some(text)) => {
                        parts.push(format!("Section '{}':\n{}", anchor, text));
                    }
                    Ok(None) => {
                        parts.push(format!("Section: {}", anchor));
                    }
                    Err(e) => tracing::warn!("Failed to read section: {}", e),
                }
            }
        }

        if let Some(figure_id) = &request.figure_id {
            parts.push(format!("Figure: {}", figure_id));
        }

        if let Some(selection) = &request.selection {
            parts.push(format!("Selected text:\n{}", selection));
        }

        parts.join("\n\n")
    }
}

fn system_prompt(mode: AgentMode) -> String {
    let base = "You are a research blog assistant. Answer based on the provided blog content.\n\
         Rules:\n\
         1. Only use information from the context.\n\
         2. If no relevant content exists, say so explicitly.\n\
         3. Cite section titles when referencing.\n\
         4. Answer in the same language as the question.";

    match mode {
        AgentMode::Reader => base.to_string(),
        AgentMode::Author => format!(
            "{}\n\
             Author mode: suggest edits using unified diff format.\n\
             Never write files directly.",
            base
        ),
    }
}

fn build_prompt(request: &ChatRequest, local: &str, results: &[SearchResult]) -> String {
    let mut prompt = String::new();

    if !local.is_empty() {
        prompt.push_str("## Local Context\n\n");
        prompt.push_str(local);
        prompt.push_str("\n\n");
    }

    if results.is_empty() {
        prompt.push_str("## Search Results\n\n(No sections matched)\n\n");
    } else {
        prompt.push_str("## Retrieved Sections\n\n");
        for r in results {
            prompt.push_str(&format!(
                "### {} / {} ({})\n{}\n\n",
                r.article_title, r.section_title, r.section_anchor, r.content
            ));
        }
    }

    prompt.push_str("## Question\n\n");
    prompt.push_str(&request.question);
    prompt
}
