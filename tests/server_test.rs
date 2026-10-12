use std::sync::Arc;
use std::time::Duration;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use openrouter_free_gateway::models::{OpenAIModel, OpenAIModelList};
use openrouter_free_gateway::proxy::{GatewayState, ModelAccessDecision, create_router};
use serde_json::Value;
use tokio::sync::RwLock;
use tower::ServiceExt;

#[test]
fn test_model_access_decision() {
    let list = OpenAIModelList {
        object: "list".to_string(),
        data: vec![OpenAIModel {
            id: "nemotron-3.5-lightning:free".to_string(),
            object: "model".to_string(),
            created: 1_700_000_000,
            owned_by: "openrouter".to_string(),
            context_length: Some(1_000_000),
            max_completion_tokens: Some(16_384),
            upstream_id: Some("nvidia/nemotron-3.5-lightning:free".to_string()),
        }],
    };

    assert_eq!(
        ModelAccessDecision::check("nemotron-3.5-lightning:free", &list),
        ModelAccessDecision::Allowed {
            upstream_model: "nvidia/nemotron-3.5-lightning:free".to_string(),
        }
    );
    assert_eq!(
        ModelAccessDecision::check("nvidia/nemotron-3.5-lightning:free", &list),
        ModelAccessDecision::Allowed {
            upstream_model: "nvidia/nemotron-3.5-lightning:free".to_string(),
        }
    );
    assert_eq!(
        ModelAccessDecision::check("openai/gpt-4o", &list),
        ModelAccessDecision::Forbidden
    );
    assert_eq!(
        ModelAccessDecision::check("anthropic/claude-3-5-sonnet", &list),
        ModelAccessDecision::Forbidden
    );
}

#[tokio::test]
async fn test_health_endpoint() {
    let state = GatewayState {
        client: reqwest::Client::new(),
        cache: Arc::new(RwLock::new(None)),
        cache_ttl: Duration::from_secs(300),
        default_api_key: None,
        openrouter_base_url: "https://openrouter.ai/api/v1".to_string(),
    };

    let app = create_router(state);

    let response = app
        .oneshot(
            Request::builder()
                .uri("/health")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body_bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let json_body: Value = serde_json::from_slice(&body_bytes).unwrap();
    assert_eq!(json_body["status"], "ok");
    assert_eq!(json_body["service"], "openrouter-free-gateway");
}

#[tokio::test]
async fn test_chat_completions_blocks_paid_model() {
    let allowed_list = OpenAIModelList {
        object: "list".to_string(),
        data: vec![OpenAIModel {
            id: "nemotron-3.5-lightning:free".to_string(),
            object: "model".to_string(),
            created: 1_700_000_000,
            owned_by: "openrouter".to_string(),
            context_length: Some(1_000_000),
            max_completion_tokens: Some(16_384),
            upstream_id: Some("nvidia/nemotron-3.5-lightning:free".to_string()),
        }],
    };

    let state = GatewayState {
        client: reqwest::Client::new(),
        cache: Arc::new(RwLock::new(Some((std::time::Instant::now(), allowed_list)))),
        cache_ttl: Duration::from_secs(300),
        default_api_key: None,
        openrouter_base_url: "https://openrouter.ai/api/v1".to_string(),
    };

    let app = create_router(state);

    let req_body = serde_json::json!({
        "model": "openai/gpt-4o",
        "messages": [{"role": "user", "content": "hi"}]
    });

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/chat/completions")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&req_body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::FORBIDDEN);
    let body_bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let json_body: Value = serde_json::from_slice(&body_bytes).unwrap();
    assert_eq!(json_body["error"]["type"], "permission_denied");
}
