use std::path::PathBuf;
use std::sync::Arc;

use anyhow::Result;
use axum::extract::{Form, Query, State};
use axum::http::{HeaderMap, HeaderValue, StatusCode};
use axum::response::{Html, IntoResponse, Response};
use axum::{Json, Router};
use serde_json::json;
use tower_http::cors::CorsLayer;

use super::oauth::{AuthRequest, OAuthStore};
use super::protocol::JsonRpcRequest;
use super::server::{dispatch_request, make_content_response};
use super::session_store::SessionStore;

#[derive(Clone)]
pub struct AppState {
    pub session_store: Arc<SessionStore>,
    pub oauth: Arc<OAuthStore>,
    pub working_dir: Arc<PathBuf>,
    pub public_url: Arc<String>,
}

pub fn build_router(state: AppState) -> Router {
    Router::new()
        .route(
            "/.well-known/oauth-authorization-server",
            axum::routing::get(handle_oauth_metadata),
        )
        .route("/authorize", axum::routing::get(handle_authorize_page))
        .route("/authorize", axum::routing::post(handle_authorize_submit))
        .route("/token", axum::routing::post(handle_token))
        .route("/mcp", axum::routing::post(handle_mcp))
        .layer(CorsLayer::permissive())
        .with_state(state)
}

pub async fn run_http_server(
    working_dir: PathBuf,
    public_url: String,
    host: String,
    port: u16,
) -> Result<()> {
    let state = AppState {
        session_store: Arc::new(SessionStore::new()),
        oauth: Arc::new(OAuthStore::new()),
        working_dir: Arc::new(working_dir),
        public_url: Arc::new(public_url.clone()),
    };
    let app = build_router(state);
    let addr = format!("{}:{}", host, port);
    eprintln!("moeb: HTTP MCP server listening on http://{}", addr);
    eprintln!("moeb: public URL: {}", public_url);
    eprintln!("moeb: OAuth authorization: {}/authorize", public_url);
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    axum::serve(listener, app).await?;
    Ok(())
}

async fn handle_oauth_metadata(State(state): State<AppState>) -> impl IntoResponse {
    let base = &*state.public_url;
    Json(json!({
        "issuer": base,
        "authorization_endpoint": format!("{}/authorize", base),
        "token_endpoint": format!("{}/token", base),
        "response_types_supported": ["code"],
        "grant_types_supported": ["authorization_code"],
        "code_challenge_methods_supported": ["S256"]
    }))
}

async fn handle_authorize_page(
    Query(params): Query<AuthRequest>,
    State(state): State<AppState>,
) -> Response {
    let resp_type = params.response_type.as_deref().unwrap_or("");
    let method = params.code_challenge_method.as_deref().unwrap_or("");
    if resp_type != "code" || method != "S256" {
        return (StatusCode::BAD_REQUEST, "invalid_request: response_type must be code and code_challenge_method must be S256").into_response();
    }
    let auth_state = match state.oauth.begin_auth(params) {
        Ok(s) => s,
        Err(e) => return (StatusCode::BAD_REQUEST, e.to_string()).into_response(),
    };
    let html = format!(
        r#"<!DOCTYPE html>
<html lang="en">
<head><meta charset="utf-8"><title>Authorise moeb</title>
<style>body{{font-family:sans-serif;max-width:480px;margin:80px auto;padding:0 16px}}
button{{padding:10px 24px;font-size:16px;cursor:pointer;background:#0070f3;color:#fff;border:none;border-radius:6px}}</style>
</head>
<body>
<h2>Authorise moeb</h2>
<p>A client is requesting access to run moeb tools in your working directory.</p>
<form method="post" action="/authorize">
  <input type="hidden" name="state" value="{state}">
  <button type="submit">Authorise moeb</button>
</form>
</body>
</html>"#,
        state = auth_state
    );
    Html(html).into_response()
}

#[derive(serde::Deserialize)]
struct AuthorizeSubmitForm {
    state: String,
}

async fn handle_authorize_submit(
    State(state): State<AppState>,
    Form(body): Form<AuthorizeSubmitForm>,
) -> Response {
    let session_id = uuid::Uuid::new_v4().to_string();
    match state.oauth.approve(&body.state, &session_id) {
        Ok((code, redirect_uri)) => {
            let location = format!("{}?code={}&state={}", redirect_uri, code, body.state);
            let mut headers = HeaderMap::new();
            if let Ok(v) = HeaderValue::from_str(&location) {
                headers.insert(axum::http::header::LOCATION, v);
            }
            (StatusCode::FOUND, headers).into_response()
        }
        Err(e) => (StatusCode::BAD_REQUEST, e.to_string()).into_response(),
    }
}

#[derive(serde::Deserialize)]
struct TokenForm {
    grant_type: Option<String>,
    code: String,
    code_verifier: String,
    client_id: String,
}

async fn handle_token(
    State(state): State<AppState>,
    Form(body): Form<TokenForm>,
) -> Response {
    if body.grant_type.as_deref() != Some("authorization_code") {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "unsupported_grant_type"})),
        )
            .into_response();
    }
    match state.oauth.exchange_code(&body.code, &body.code_verifier, &body.client_id) {
        Ok(token) => Json(json!({
            "access_token": token,
            "token_type": "Bearer",
            "expires_in": 86400
        }))
        .into_response(),
        Err(_) => (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "invalid_grant"})),
        )
            .into_response(),
    }
}

async fn handle_mcp(
    State(state): State<AppState>,
    headers: HeaderMap,
    body: axum::body::Bytes,
) -> Response {
    let token = match extract_bearer_token(&headers) {
        Some(t) => t,
        None => return StatusCode::UNAUTHORIZED.into_response(),
    };
    if !state.oauth.validate_token(&token) {
        return StatusCode::UNAUTHORIZED.into_response();
    }

    let session_id_opt = headers
        .get("mcp-session-id")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string());

    let working_dir = state.working_dir.as_ref().clone();
    let (session_id, _is_new) =
        state.session_store.get_or_create(session_id_opt.as_deref(), &working_dir);

    let request: JsonRpcRequest = match serde_json::from_slice(&body) {
        Ok(r) => r,
        Err(e) => {
            let resp = make_content_response(json!(null), format!("Parse error: {}", e));
            return json_response(resp, &session_id);
        }
    };

    if request.method == "notifications/initialized" || request.id.is_none() {
        return (StatusCode::NO_CONTENT, HeaderMap::new()).into_response();
    }

    let response = tokio::task::block_in_place(|| {
        state.session_store.with_session(&session_id, |entry| {
            dispatch_request(&request, &mut entry.session, &mut entry.executor)
        })
    });

    match response {
        Some(resp) => json_response(resp, &session_id),
        None => (StatusCode::INTERNAL_SERVER_ERROR, "session not found").into_response(),
    }
}

fn extract_bearer_token(headers: &HeaderMap) -> Option<String> {
    let auth = headers.get(axum::http::header::AUTHORIZATION)?.to_str().ok()?;
    auth.strip_prefix("Bearer ").map(|s| s.to_string())
}

fn json_response(resp: super::protocol::JsonRpcResponse, session_id: &str) -> Response {
    let mut headers = HeaderMap::new();
    if let Ok(v) = HeaderValue::from_str(session_id) {
        headers.insert("mcp-session-id", v);
    }
    let body = serde_json::to_string(&resp).unwrap_or_default();
    let mut response = (
        StatusCode::OK,
        [(axum::http::header::CONTENT_TYPE, "application/json")],
        body,
    )
        .into_response();
    response.headers_mut().extend(headers);
    response
}

#[cfg(test)]
#[path = "http_server_tests.rs"]
mod tests;
