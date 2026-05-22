use anyhow::{bail, Context, Result};
use std::path::Path;
use std::process::Command;

/// Sanitise a string so it is safe for use in a Conventional Branch description segment:
/// lowercase, alphanumerics and hyphens only, no consecutive/leading/trailing hyphens.
fn to_branch_description(s: &str) -> String {
    // Replace any run of non-alphanumeric chars with a single hyphen, lowercase everything.
    let mut out = String::new();
    let mut last_was_hyphen = false;
    for ch in s.chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch.to_ascii_lowercase());
            last_was_hyphen = false;
        } else if !last_was_hyphen && !out.is_empty() {
            // Emit a hyphen in place of any separator run, but not at the start.
            out.push('-');
            last_was_hyphen = true;
        }
    }
    // Strip trailing hyphen if present.
    if out.ends_with('-') {
        out.pop();
    }
    out
}

/// Create a Conventional Branch-compliant `feat/<domain>-<slug>` branch.
/// Returns the branch name that was created.
/// Must be called from the repository root (CWD must contain the `.git/` directory).
pub fn create_spec_branch(domain: &str, slug: &str) -> Result<String> {
    let desc = format!("{}-{}", to_branch_description(domain), to_branch_description(slug));
    let branch = format!("feat/{}", desc);

    eprintln!("[moeb] creating branch: {}", branch);

    let output = Command::new("git")
        .args(["checkout", "-b", &branch])
        .output()
        .context("failed to invoke git — is git installed and on PATH?")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!("git checkout -b {} failed: {}", branch, stderr.trim());
    }

    Ok(branch)
}

/// Stage the spec file and README, then create a Conventional Commits message commit.
/// `spec_path` and `readme_path` must be relative to the repository root.
pub fn commit_spec(spec_path: &Path, readme_path: &Path, domain: &str, slug: &str) -> Result<String, String> {
    git_add_files(&[spec_path, readme_path])?;
    let message = format!("docs({}): add {} specification", domain, slug);
    git_commit_with_message(&message)
}

/// Stage all working-tree changes and create a feat commit for the run.
pub fn commit_run(domain: &str, slug: &str) -> Result<String, String> {
    git_stage_all()?;
    let message = format!("feat({}): execute {} specification", domain, slug);
    git_commit_with_message(&message)
}

fn git_add_files(paths: &[&Path]) -> Result<(), String> {
    let mut cmd = Command::new("git");
    cmd.arg("add");
    for p in paths {
        cmd.arg(p.as_os_str());
    }
    let output = cmd
        .output()
        .map_err(|e| format!("failed to invoke git add: {}", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("git add failed: {}", stderr.trim()));
    }
    Ok(())
}

fn git_commit_with_message(message: &str) -> Result<String, String> {
    eprintln!("[moeb] committing: {}", message);

    let output = Command::new("git")
        .args(["commit", "-m", message])
        .output()
        .map_err(|e| format!("failed to invoke git commit: {}", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("git commit failed: {}", stderr.trim()));
    }
    Ok(format!("Committed: {}", message))
}

fn git_stage_all() -> Result<(), String> {
    eprintln!("[moeb] staging all changes");

    let output = Command::new("git")
        .args(["add", "-A"])
        .output()
        .map_err(|e| format!("failed to invoke git add -A: {}", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("git add -A failed: {}", stderr.trim()));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::to_branch_description;

    #[test]
    fn lowercase_and_hyphens() {
        assert_eq!(to_branch_description("Hello World"), "hello-world");
    }

    #[test]
    fn strips_leading_trailing_hyphens() {
        assert_eq!(to_branch_description("_foo_"), "foo");
    }

    #[test]
    fn no_consecutive_hyphens() {
        assert_eq!(to_branch_description("foo--bar"), "foo-bar");
        assert_eq!(to_branch_description("foo  bar"), "foo-bar");
    }

    #[test]
    fn alphanumeric_passthrough() {
        assert_eq!(to_branch_description("spec123"), "spec123");
    }

    #[test]
    fn empty_string() {
        assert_eq!(to_branch_description(""), "");
    }

    #[test]
    fn domain_slug_combination() {
        assert_eq!(
            format!("feat/{}-{}", to_branch_description("vcs"), to_branch_description("my-spec")),
            "feat/vcs-my-spec"
        );
    }

    #[test]
    fn branch_name_is_conventional_branch_compliant() {
        // Verify the full branch name produced for the actual spec that introduced this rule.
        let domain = "vcs";
        let slug = "spec-creation-branch-commit-format";
        let desc = format!("{}-{}", to_branch_description(domain), to_branch_description(slug));
        let branch = format!("feat/{}", desc);

        assert_eq!(branch, "feat/vcs-spec-creation-branch-commit-format");
        assert!(branch.starts_with("feat/"), "type prefix must be feat/");

        let description = &branch["feat/".len()..];
        assert!(!description.is_empty(), "description must not be empty");
        assert!(!description.starts_with('-'), "description must not start with hyphen");
        assert!(!description.ends_with('-'), "description must not end with hyphen");
        assert!(!description.contains("--"), "description must not have consecutive hyphens");
        assert!(
            description.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-'),
            "description must only contain lowercase alphanumeric characters and hyphens"
        );
    }

    #[test]
    fn commit_message_is_conventional_commits_compliant() {
        // Verify the format string used in commit_spec produces a valid Conventional Commits message.
        let domain = "vcs";
        let slug = "spec-creation-branch-commit-format";
        let message = format!("docs({}): add {} specification", domain, slug);

        assert_eq!(message, "docs(vcs): add spec-creation-branch-commit-format specification");
        // type must be `docs`
        assert!(message.starts_with("docs("), "commit type must be docs");
        // scope must be the domain name in parentheses followed by colon-space
        assert!(message.contains(&format!("docs({}):", domain)), "commit scope must be domain");
        // separator must be colon + single space
        assert!(message.contains("): "), "separator must be colon-space");
        // description must be non-empty after the separator
        let after_sep = message.split("): ").nth(1).unwrap_or("");
        assert!(!after_sep.is_empty(), "commit description must be non-empty");
        // subject line must be ≤ 72 characters
        assert!(
            message.len() <= 72,
            "subject line must be ≤ 72 characters, got {}",
            message.len()
        );
    }
}
