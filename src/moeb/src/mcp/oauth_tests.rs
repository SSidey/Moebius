use std::time::{Duration, Instant};

use super::{OAuthStore, AuthRequest, TokenInfo};
use sha2::{Digest, Sha256};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};

fn make_pkce_pair() -> (String, String) {
    let verifier = "test_code_verifier_that_is_long_enough_abcdefghij".to_string();
    let hash = Sha256::digest(verifier.as_bytes());
    let challenge = URL_SAFE_NO_PAD.encode(hash);
    (verifier, challenge)
}

#[test]
fn pkce_valid_verifier_exchanges_successfully() {
    let store = OAuthStore::new();
    let (verifier, challenge) = make_pkce_pair();

    let auth_state = store.begin_auth(AuthRequest {
        response_type: Some("code".to_string()),
        client_id: "test-client".to_string(),
        redirect_uri: "http://localhost/callback".to_string(),
        code_challenge: challenge,
        code_challenge_method: Some("S256".to_string()),
        state: None,
    }).unwrap();

    let (code, _redirect) = store.approve(&auth_state, "session-1").unwrap();
    let token = store.exchange_code(&code, &verifier, "test-client").unwrap();
    assert!(!token.is_empty());
    assert!(store.validate_token(&token));
}

#[test]
fn pkce_wrong_verifier_returns_error() {
    let store = OAuthStore::new();
    let (_verifier, challenge) = make_pkce_pair();

    let auth_state = store.begin_auth(AuthRequest {
        response_type: Some("code".to_string()),
        client_id: "test-client".to_string(),
        redirect_uri: "http://localhost/callback".to_string(),
        code_challenge: challenge,
        code_challenge_method: Some("S256".to_string()),
        state: None,
    }).unwrap();

    let (code, _) = store.approve(&auth_state, "session-1").unwrap();
    let result = store.exchange_code(&code, "wrong_verifier", "test-client");
    assert!(result.is_err(), "expected PKCE failure");
}

#[test]
fn expired_token_fails_validation() {
    let store = OAuthStore::new();
    let expired = TokenInfo {
        session_id: "s1".to_string(),
        expires_at: Instant::now() - Duration::from_secs(1),
    };
    store.tokens.lock().unwrap().insert("expired-token".to_string(), expired);
    assert!(!store.validate_token("expired-token"));
}
