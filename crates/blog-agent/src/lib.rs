use anyhow::Result;
use axum::{
    extract::{Json, State},
    http::StatusCode,
    response::{IntoResponse, Response, Sse},
    routing::{get, post},
    Router,
};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;
use tower_http::cors::CorsLayer;

pub mod agent;
pub mod llm;
pub mod retrieval;

use agent::{AgentCore, ChatRequest};
use llm::LlmConfig;

#[derive(Clone)]
pub struct AppState {
    pub agent: Arc<RwLock<AgentCore>>,
}

pub async fn create_app(index_dir: PathBuf, llm_config: Option<LlmConfig>) -> Result<Router> {
    let agent = AgentCore::new(index_dir, llm_config)?;

    let state = AppState {
        agent: Arc::new(RwLock::new(agent)),
    };

    let app = Router::new()
        .route("/api/health", get(health_handler))
        .route("/api/chat", post(chat_handler))
        .with_state(state)
        .layer(CorsLayer::permissive());

    Ok(app)
}

async fn health_handler() -> impl IntoResponse {
    Json(serde_json::json!({
        "status": "ok",
        "service": "blog-agent"
    }))
}

async fn chat_handler(State(state): State<AppState>, Json(request): Json<ChatRequest>) -> Response {
    const MAX_QUESTION_LEN: usize = 2000;
    const MAX_SELECTION_LEN: usize = 5000;

    if request.question.len() > MAX_QUESTION_LEN {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({
                "error": format!("Question too long (max {} chars)", MAX_QUESTION_LEN)
            })),
        )
            .into_response();
    }

    if let Some(ref sel) = request.selection {
        if sel.len() > MAX_SELECTION_LEN {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({
                    "error": format!("Selection too long (max {} chars)", MAX_SELECTION_LEN)
                })),
            )
                .into_response();
        }
    }

    let agent = state.agent.read().await;

    match agent.chat(&request).await {
        Ok(response_stream) => Sse::new(response_stream).into_response(),
        Err(e) => {
            tracing::error!("Chat error: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({
                    "error": e.to_string()
                })),
            )
                .into_response()
        }
    }
}
