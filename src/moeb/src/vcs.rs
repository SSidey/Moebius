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

/// Create a Conventional Branch-compliant `chore/<domain>-<slug>` branch.
/// Returns the branch name that was created.
/// Must be called from the repository root (CWD must contain the `.git/` directory).
pub fn create_spec_branch(domain: &str, slug: &str) -> Result<String> {
    let desc = format!("{}-{}", to_branch_description(domain), to_branch_description(slug));
    let branch = format!("chore/{}", desc);

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
pub fn commit_spec(spec_path: &Path, readme_path: &Path, domain: &str, slug: &str) -> Result<()> {
    let spec_str = spec_path
        .to_str()
        .context("spec path contains non-UTF-8 characters")?;
    let readme_str = readme_path
        .to_str()
        .context("README path contains non-UTF-8 characters")?;

    eprintln!("[moeb] staging: {} and {}", spec_str, readme_str);

    let add_output = Command::new("git")
        .args(["add", spec_str, readme_str])
        .output()
        .context("failed to invoke git add")?;

    if !add_output.status.success() {
        let stderr = String::from_utf8_lossy(&add_output.stderr);
        bail!("git add failed: {}", stderr.trim());
    }

    let message = format!("docs({}): add {} specification", domain, slug);
    eprintln!("[moeb] committing: {}", message);

    let commit_output = Command::new("git")
        .args(["commit", "-m", &message])
        .output()
        .context("failed to invoke git commit")?;

    if !commit_output.status.success() {
        let stderr = String::from_utf8_lossy(&commit_output.stderr);
        bail!("git commit failed: {}", stderr.trim());
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
            format!("chore/{}-{}", to_branch_description("vcs"), to_branch_description("my-spec")),
            "chore/vcs-my-spec"
        );
    }
}
