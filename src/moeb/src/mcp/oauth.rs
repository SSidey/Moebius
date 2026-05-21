use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use anyhow::Result;
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use sha2::{Digest, Sha256};
use uuid::Uuid;

pub struct PendingAuth {
    pub client_id: String,
    pub redirect_uri: String,
    pub code_challenge: String,
    pub state: String,
}

pub struct IssuedCode {
    pub code_challenge: String,
    pub client_id: String,
    pub redirect_uri: String,
    pub issued_at: Instant,
}

pub struct TokenInfo {
    pub session_id: String,
    pub expires_at: Instant,
}

#[derive(serde::Deserialize)]
pub struct AuthRequest {
    pub response_type: Option<String>,
    pub client_id: String,
    pub redirect_uri: String,
    pub code_challenge: String,
    pub code_challenge_method: Option<String>,
    pub state: Option<String>,
}

pub struct OAuthStore {
    pending: Mutex<HashMap<String, PendingAuth>>,
    codes: Mutex<HashMap<String, IssuedCode>>,
    tokens: Mutex<HashMap<String, TokenInfo>>,
}

impl OAuthStore {
    pub fn new() -> Self {
        Self {
            pending: Mutex::new(HashMap::new()),
            codes: Mutex::new(HashMap::new()),
            tokens: Mutex::new(HashMap::new()),
        }
    }

    pub fn begin_auth(&self, params: AuthRequest) -> Result<String> {
        let method = params.code_challenge_method.as_deref().unwrap_or("");
        if method != "S256" {
            anyhow::bail!("code_challenge_method must be S256");
        }
        let state = params.state.unwrap_or_else(|| Uuid::new_v4().to_string());
        self.pending.lock().unwrap().insert(
            state.clone(),
            PendingAuth {
                client_id: params.client_id,
                redirect_uri: params.redirect_uri,
                code_challenge: params.code_challenge,
                state: state.clone(),
            },
        );
        Ok(state)
    }

    pub fn approve(&self, state: &str, _session_id: &str) -> Result<(String, String)> {
        let pending = self
            .pending
            .lock()
            .unwrap()
            .remove(state)
            .ok_or_else(|| anyhow::anyhow!("unknown state"))?;
        let code = Uuid::new_v4().to_string();
        self.codes.lock().unwrap().insert(
            code.clone(),
            IssuedCode {
                code_challenge: pending.code_challenge,
                client_id: pending.client_id,
                redirect_uri: pending.redirect_uri.clone(),
                issued_at: Instant::now(),
            },
        );
        Ok((code, pending.redirect_uri))
    }

    pub fn exchange_code(
        &self,
        code: &str,
        code_verifier: &str,
        client_id: &str,
    ) -> Result<String> {
        self.purge_expired();
        let issued = self
            .codes
            .lock()
            .unwrap()
            .remove(code)
            .ok_or_else(|| anyhow::anyhow!("invalid_grant: unknown or expired code"))?;

        if issued.client_id != client_id {
            anyhow::bail!("invalid_grant: client_id mismatch");
        }

        let hash = Sha256::digest(code_verifier.as_bytes());
        let computed = URL_SAFE_NO_PAD.encode(hash);
        if computed != issued.code_challenge {
            anyhow::bail!("invalid_grant: PKCE verification failed");
        }

        let token = Uuid::new_v4().to_string();
        self.tokens.lock().unwrap().insert(
            token.clone(),
            TokenInfo {
                session_id: Uuid::new_v4().to_string(),
                expires_at: Instant::now() + Duration::from_secs(86400),
            },
        );
        Ok(token)
    }

    pub fn validate_token(&self, token: &str) -> bool {
        self.purge_expired();
        let tokens = self.tokens.lock().unwrap();
        tokens.get(token).map(|t| t.expires_at > Instant::now()).unwrap_or(false)
    }

    pub fn purge_expired(&self) {
        let now = Instant::now();
        self.codes
            .lock()
            .unwrap()
            .retain(|_, v| v.issued_at + Duration::from_secs(60) > now);
        self.tokens.lock().unwrap().retain(|_, v| v.expires_at > now);
    }
}

#[cfg(test)]
#[path = "oauth_tests.rs"]
mod tests;
