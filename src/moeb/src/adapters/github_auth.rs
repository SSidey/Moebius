use jsonwebtoken::{encode, Algorithm, EncodingKey, Header};
use serde::Serialize;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Serialize)]
struct AppClaims {
    iat: u64,
    exp: u64,
    iss: String,
}

/// Generate a GitHub App JWT valid for ~10 minutes.
pub fn generate_app_jwt(app_id: u64, private_key_pem: &str) -> Result<String, String> {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let claims = AppClaims {
        iat: now.saturating_sub(60),
        exp: now + 600,
        iss: app_id.to_string(),
    };
    let key = EncodingKey::from_rsa_pem(private_key_pem.as_bytes())
        .map_err(|e| format!("PEM decode error: {e}"))?;
    encode(&Header::new(Algorithm::RS256), &claims, &key)
        .map_err(|e| format!("JWT encode error: {e}"))
}

/// Exchange an App JWT for a short-lived installation access token.
pub async fn fetch_installation_token(
    installation_id: u64,
    jwt: &str,
    client: &reqwest::Client,
) -> Result<String, String> {
    let url = format!(
        "https://api.github.com/app/installations/{installation_id}/access_tokens"
    );
    let resp = client
        .post(&url)
        .bearer_auth(jwt)
        .header("Accept", "application/vnd.github+json")
        .header("X-GitHub-Api-Version", "2022-11-28")
        .header("User-Agent", "moeb-signal-router/1.0")
        .send()
        .await
        .map_err(|e| e.to_string())?;
    let body: serde_json::Value = resp.json().await.map_err(|e| e.to_string())?;
    body["token"]
        .as_str()
        .map(String::from)
        .ok_or_else(|| "token field absent in installation token response".into())
}
