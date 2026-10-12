use std::sync::Arc;
use std::time::{Duration, Instant};

use axum::body::Body;
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use futures_util::TryStreamExt;
use serde::Deserialize;
use serde_json::{Value, json};
use tokio::sync::RwLock;
use tracing::{error, info, warn};

use crate::models::{OpenAIModelList, OpenRouterModel, filter_free_models};

#[derive(Clone)]
pub struct GatewayState {
    pub client: reqwest::Client,
    pub cache: Arc<RwLock<Option<(Instant, OpenAIModelList)>>>,
    pub cache_ttl: Duration,
    pub default_api_key: Option<String>,
    pub openrouter_base_url: String,
}

#[derive(Debug, Deserialize)]
struct OpenRouterModelsResponse {
    data: Vec<OpenRouterModel>,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ModelAccessDecision {
    Allowed { upstream_model: String },
    Forbidden,
}

impl ModelAccessDecision {
    #[must_use]
    pub fn check(model: &str, allowed_models: &OpenAIModelList) -> Self {
        for allowed in &allowed_models.data {
            let matches_id = allowed.id == model;
            let matches_upstream = allowed.upstream_id.as_deref() == Some(model);
            if matches_id || matches_upstream {
                let upstream = allowed
                    .upstream_id
                    .clone()
                    .unwrap_or_else(|| allowed.id.clone());
                return Self::Allowed {
                    upstream_model: upstream,
                };
            }
        }
        Self::Forbidden
    }
}

pub fn create_router(state: GatewayState) -> Router {
    Router::new()
        .route("/health", get(health_handler))
        .route("/v1/models", get(list_models_handler))
        .route("/models", get(list_models_handler))
        .route("/v1/chat/completions", post(chat_completions_handler))
        .route("/chat/completions", post(chat_completions_handler))
        .with_state(state)
}

async fn health_handler() -> impl IntoResponse {
    Json(json!({
        "status": "ok",
        "service": "openrouter-free-gateway"
    }))
}

async fn fetch_or_get_cached_models(
    state: &GatewayState,
) -> Result<OpenAIModelList, (StatusCode, Json<Value>)> {
    // Check read cache first
    {
        let cache_guard = state.cache.read().await;
        match &*cache_guard {
            Some((cached_at, models)) if cached_at.elapsed() < state.cache_ttl => {
                return Ok(models.clone());
            }
            _ => {}
        }
    }

    // Cache expired or empty: fetch from OpenRouter
    let url = format!("{}/models", state.openrouter_base_url.trim_end_matches('/'));
    info!(target = "gateway", "Fetching live models from {}", url);

    let mut req = state
        .client
        .get(&url)
        .header("User-Agent", "openrouter-free-gateway/0.1.0");
    if let Some(key) = &state.default_api_key {
        req = req.bearer_auth(key);
    }

    let response = req.send().await.map_err(|err| {
        error!(
            target = "gateway",
            "Failed to fetch models from OpenRouter: {}", err
        );
        (
            StatusCode::BAD_GATEWAY,
            Json(json!({
                "error": {
                    "message": format!("Failed to connect to OpenRouter: {err}"),
                    "type": "bad_gateway"
                }
            })),
        )
    })?;

    match response.status().is_success() {
        true => {
            let body: OpenRouterModelsResponse = response.json().await.map_err(|err| {
                error!(
                    target = "gateway",
                    "Failed to parse OpenRouter models JSON: {}", err
                );
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({
                        "error": {
                            "message": format!("Invalid OpenRouter models payload: {err}"),
                            "type": "internal_server_error"
                        }
                    })),
                )
            })?;

            let filtered_models = filter_free_models(&body.data);
            info!(
                target = "gateway",
                "Discovered {} total models; filtered to {} 100% free models",
                body.data.len(),
                filtered_models.len()
            );

            let model_list = OpenAIModelList {
                object: "list".to_string(),
                data: filtered_models,
            };

            let mut cache_guard = state.cache.write().await;
            *cache_guard = Some((Instant::now(), model_list.clone()));
            Ok(model_list)
        }
        false => {
            let status = response.status();
            let err_text = response.text().await.unwrap_or_default();
            error!(
                target = "gateway",
                "OpenRouter models endpoint returned {}: {}", status, err_text
            );
            Err((
                StatusCode::BAD_GATEWAY,
                Json(json!({
                    "error": {
                        "message": format!("OpenRouter returned HTTP {status}: {err_text}"),
                        "type": "upstream_error"
                    }
                })),
            ))
        }
    }
}

async fn list_models_handler(
    State(state): State<GatewayState>,
) -> Result<Json<OpenAIModelList>, (StatusCode, Json<Value>)> {
    let models = fetch_or_get_cached_models(&state).await?;
    Ok(Json(models))
}

async fn chat_completions_handler(
    State(state): State<GatewayState>,
    headers: HeaderMap,
    Json(payload): Json<Value>,
) -> Response {
    let requested_model = match payload.get("model").and_then(Value::as_str) {
        Some(m) => m,
        None => {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({
                    "error": {
                        "message": "Missing 'model' parameter in request",
                        "type": "invalid_request_error"
                    }
                })),
            )
                .into_response();
        }
    };

    // Ensure models list is populated
    let available_models = match fetch_or_get_cached_models(&state).await {
        Ok(models) => models,
        Err((status, json_err)) => return (status, json_err).into_response(),
    };

    // Check whitelist using ModelAccessDecision
    let upstream_model = match ModelAccessDecision::check(requested_model, &available_models) {
        ModelAccessDecision::Allowed { upstream_model } => upstream_model,
        ModelAccessDecision::Forbidden => {
            warn!(
                target = "gateway",
                "Blocked request for non-free model '{}'", requested_model
            );
            return (
                StatusCode::FORBIDDEN,
                Json(json!({
                    "error": {
                        "message": format!(
                            "Model '{requested_model}' is not in the verified 100% free models list."
                        ),
                        "type": "permission_denied"
                    }
                })),
            )
                .into_response();
        }
    };

    let mut forwarded_payload = payload;
    forwarded_payload["model"] = json!(upstream_model);
    // Resolve Authorization token
    let auth_header_value = match headers.get(header::AUTHORIZATION) {
        Some(val) => match val.to_str() {
            Ok(s) if !s.contains("dummy") && s.starts_with("Bearer ") => Some(s.to_string()),
            _ => None,
        },
        None => None,
    };

    let effective_auth = match auth_header_value {
        Some(header_token) => Some(header_token),
        None => state
            .default_api_key
            .as_ref()
            .map(|k| format!("Bearer {k}")),
    };

    let target_url = format!(
        "{}/chat/completions",
        state.openrouter_base_url.trim_end_matches('/')
    );
    let mut req = state
        .client
        .post(&target_url)
        .header(header::CONTENT_TYPE, "application/json")
        .header("User-Agent", "openrouter-free-gateway/0.1.0");

    if let Some(auth) = effective_auth {
        req = req.header(header::AUTHORIZATION, auth);
    }

    let upstream_res = match req.json(&forwarded_payload).send().await {
        Ok(res) => res,
        Err(err) => {
            error!(
                target = "gateway",
                "Failed to forward request to OpenRouter: {}", err
            );
            return (
                StatusCode::BAD_GATEWAY,
                Json(json!({
                    "error": {
                        "message": format!("Upstream network error: {err}"),
                        "type": "bad_gateway"
                    }
                })),
            )
                .into_response();
        }
    };

    let status = StatusCode::from_u16(upstream_res.status().as_u16())
        .unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);

    let mut response_builder = Response::builder().status(status);

    if let Some(content_type) = upstream_res.headers().get(header::CONTENT_TYPE) {
        response_builder = response_builder.header(header::CONTENT_TYPE, content_type);
    }

    let stream = upstream_res
        .bytes_stream()
        .map_err(|err| std::io::Error::other(format!("Stream error from upstream: {err}")));

    match response_builder.body(Body::from_stream(stream)) {
        Ok(resp) => resp,
        Err(err) => {
            error!(
                target = "gateway",
                "Failed to construct response body: {}", err
            );
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "error": {
                        "message": format!("Internal response error: {err}"),
                        "type": "internal_error"
                    }
                })),
            )
                .into_response()
        }
    }
}
