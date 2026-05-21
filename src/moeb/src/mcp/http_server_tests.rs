use std::path::PathBuf;
use std::sync::Arc;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use tower::ServiceExt;

use super::{AppState, build_router};
use crate::mcp::oauth::OAuthStore;
use crate::mcp::session_store::SessionStore;
use sha2::{Digest, Sha256};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};

fn test_state(dir: &tempfile::TempDir) -> AppState {
    AppState {
        session_store: Arc::new(SessionStore::new()),
        oauth: Arc::new(OAuthStore::new()),
        working_dir: Arc::new(PathBuf::from(dir.path())),
        public_url: Arc::new("https://example.com".to_string()),
    }
}

#[tokio::test]
async fn oauth_metadata_contains_required_fields() {
    let dir = tempfile::TempDir::new().unwrap();
    let app = build_router(test_state(&dir));
    let req = Request::builder()
        .uri("/.well-known/oauth-authorization-server")
        .body(Body::empty())
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert!(json.get("authorization_endpoint").is_some());
    assert!(json.get("token_endpoint").is_some());
    assert!(json.get("code_challenge_methods_supported").is_some());
}

#[tokio::test]
async fn mcp_request_without_token_returns_401() {
    let dir = tempfile::TempDir::new().unwrap();
    let app = build_router(test_state(&dir));
    let req = Request::builder()
        .method("POST")
        .uri("/mcp")
        .header("content-type", "application/json")
        .body(Body::from(r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}"#))
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn mcp_initialize_with_valid_token_returns_protocol_version() {
    let dir = tempfile::TempDir::new().unwrap();
    let state = test_state(&dir);

    // Complete the PKCE flow to get a real token
    let verifier = "test_verifier_long_enough_for_pkce_abcdefghij";
    let hash = Sha256::digest(verifier.as_bytes());
    let challenge = URL_SAFE_NO_PAD.encode(hash);

    let auth_state = state.oauth.begin_auth(crate::mcp::oauth::AuthRequest {
        response_type: Some("code".to_string()),
        client_id: "c1".to_string(),
        redirect_uri: "http://localhost/cb".to_string(),
        code_challenge: challenge,
        code_challenge_method: Some("S256".to_string()),
        state: None,
    }).unwrap();
    let (code, _) = state.oauth.approve(&auth_state, "sid").unwrap();
    let token = state.oauth.exchange_code(&code, verifier, "c1").unwrap();

    let app = build_router(state);
    let req = Request::builder()
        .method("POST")
        .uri("/mcp")
        .header("content-type", "application/json")
        .header("authorization", format!("Bearer {}", token))
        .body(Body::from(r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}"#))
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["result"]["protocolVersion"], "2024-11-05");
}
