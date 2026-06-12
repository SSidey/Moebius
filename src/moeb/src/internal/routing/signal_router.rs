use crate::adapters::github_auth::{fetch_installation_token, generate_app_jwt};
use crate::internal::routing::constants::{
    MOEB_GITHUB_APP_ID, MOEB_GITHUB_APP_INSTALLATION_ID, MOEB_GITHUB_APP_PRIVATE_KEY_PEM,
};
use serde_json::Value;

/// Spawn a background task to route a moeb-source signal to GitHub Issues.
/// Returns immediately; all GitHub API calls are non-blocking and silent on failure.
pub fn spawn_github_signal_routing(
    signal: Value,
    originating_project: String,
    repo_slug: String,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let _ = route_signal(signal, originating_project, repo_slug).await;
    })
}

async fn route_signal(
    signal: Value,
    originating_project: String,
    repo_slug: String,
) -> Result<(), String> {
    let pem = MOEB_GITHUB_APP_PRIVATE_KEY_PEM;
    if pem.is_empty() {
        return Ok(()); // Constants not populated; routing disabled in dev builds.
    }
    let jwt = generate_app_jwt(MOEB_GITHUB_APP_ID, pem)?;
    let client = reqwest::Client::new();
    let token =
        fetch_installation_token(MOEB_GITHUB_APP_INSTALLATION_ID, &jwt, &client).await?;
    let title = signal["title"].as_str().unwrap_or_default();
    let (owner, repo) = repo_slug
        .split_once('/')
        .ok_or_else(|| format!("invalid repo_slug: {repo_slug}"))?;
    if let Some(issue_number) = search_issue(&client, &token, owner, repo, title).await? {
        add_comment(&client, &token, owner, repo, issue_number, &signal, &originating_project)
            .await
    } else {
        create_issue(&client, &token, owner, repo, &signal, &originating_project).await
    }
}

async fn search_issue(
    client: &reqwest::Client,
    token: &str,
    owner: &str,
    repo: &str,
    title: &str,
) -> Result<Option<u64>, String> {
    let q = format!("repo:{owner}/{repo} in:title is:issue is:open \"[signal] {title}\"");
    let resp = client
        .get("https://api.github.com/search/issues")
        .bearer_auth(token)
        .query(&[("q", q.as_str()), ("per_page", "1")])
        .header("Accept", "application/vnd.github+json")
        .header("X-GitHub-Api-Version", "2022-11-28")
        .header("User-Agent", "moeb-signal-router/1.0")
        .send()
        .await
        .map_err(|e| e.to_string())?;
    let body: Value = resp.json().await.map_err(|e| e.to_string())?;
    Ok(body["items"]
        .as_array()
        .and_then(|a| a.first())
        .and_then(|i| i["number"].as_u64()))
}

async fn add_comment(
    client: &reqwest::Client,
    token: &str,
    owner: &str,
    repo: &str,
    issue_number: u64,
    signal: &Value,
    originating_project: &str,
) -> Result<(), String> {
    let url = format!(
        "https://api.github.com/repos/{owner}/{repo}/issues/{issue_number}/comments"
    );
    let run_id = signal
        .get("occurrences")
        .and_then(|o| o.as_array())
        .and_then(|a| a.last())
        .and_then(|o| o["run_id"].as_str())
        .unwrap_or("-");
    let body = format!(
        "**Occurrence recorded**\n\n\
        - **Originating project**: {originating_project}\n\
        - **run_id**: {run_id}\n\
        - **category**: {}\n\
        - **severity**: {}\n\
        - **timestamp**: {}",
        signal["category"].as_str().unwrap_or("-"),
        signal["severity"].as_str().unwrap_or("-"),
        signal["last_seen"].as_str().unwrap_or("-"),
    );
    client
        .post(&url)
        .bearer_auth(token)
        .header("Accept", "application/vnd.github+json")
        .header("X-GitHub-Api-Version", "2022-11-28")
        .header("User-Agent", "moeb-signal-router/1.0")
        .json(&serde_json::json!({ "body": body }))
        .send()
        .await
        .map_err(|e| e.to_string())?;
    Ok(())
}

async fn create_issue(
    client: &reqwest::Client,
    token: &str,
    owner: &str,
    repo: &str,
    signal: &Value,
    originating_project: &str,
) -> Result<(), String> {
    let url = format!("https://api.github.com/repos/{owner}/{repo}/issues");
    let raw_title = signal["title"].as_str().unwrap_or("unknown");
    let title = format!("[signal] {raw_title}");
    let body = format!(
        "**Category**: {}\n**Severity**: {}\n**Originating project**: {originating_project}\n\n\
        **Description**:\n{}\n\n**Proposed Resolution**:\n{}",
        signal["category"].as_str().unwrap_or("-"),
        signal["severity"].as_str().unwrap_or("-"),
        signal["description"].as_str().unwrap_or("-"),
        signal["proposed_resolution"].as_str().unwrap_or("-"),
    );
    let category_label = signal["category"]
        .as_str()
        .unwrap_or("unknown")
        .to_lowercase()
        .replace(' ', "-");
    let severity_label = signal["severity"]
        .as_str()
        .unwrap_or("unknown")
        .to_lowercase();
    client
        .post(&url)
        .bearer_auth(token)
        .header("Accept", "application/vnd.github+json")
        .header("X-GitHub-Api-Version", "2022-11-28")
        .header("User-Agent", "moeb-signal-router/1.0")
        .json(&serde_json::json!({
            "title": title,
            "body": body,
            "labels": ["moeb-signal", category_label, severity_label],
        }))
        .send()
        .await
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[cfg(test)]
#[path = "signal_router_tests.rs"]
mod tests;
