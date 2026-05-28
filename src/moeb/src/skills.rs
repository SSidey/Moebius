use std::path::Path;

/// Returns the body of a skill file with YAML frontmatter stripped.
fn strip_skill_frontmatter(content: &str) -> String {
    if let Some(body) = content.strip_prefix("---\n") {
        if let Some(end) = body.find("\n---\n") {
            return body[end + 5..].to_string();
        }
        if let Some(end) = body.find("\n---") {
            let rest = &body[end + 4..];
            if rest.is_empty() || rest.starts_with('\n') {
                return rest.trim_start_matches('\n').to_string();
            }
        }
    }
    content.to_string()
}

/// Extracts the `review:` bool from skill file frontmatter. Returns `true` when absent.
pub fn extract_review_flag(content: &str) -> bool {
    let body = match content.strip_prefix("---\n") {
        Some(b) => b,
        None => return true,
    };
    let end = match body.find("\n---") {
        Some(e) => e,
        None => return true,
    };
    let yaml_str = &body[..end];
    for line in yaml_str.lines() {
        if let Some(val) = line.strip_prefix("review:") {
            return val.trim() != "false";
        }
    }
    true
}

/// Resolves and returns the body of the named skill file (frontmatter stripped).
///
/// Resolution order:
///   1. {moeb_dir}/skills/{name}.skill.md  (project-local override)
///   2. Binary-bundled asset skills/{name}.skill.md
///   3. Empty string with a stderr warning
///
/// Returns `Err` if `name` is a protected baseline skill and a project-level override
/// file exists at `{moeb_dir}/skills/{name}.skill.md`.
pub fn load_skill(moeb_dir: &Path, name: &str) -> anyhow::Result<String> {
    use crate::internal::constants::PROTECTED_SKILLS;

    let local_path = moeb_dir.join("skills").join(format!("{}.skill.md", name));
    if PROTECTED_SKILLS.contains(&name) && local_path.exists() {
        return Err(anyhow::anyhow!(
            ".moeb/skills/{name}.skill.md overrides a protected baseline skill.\n\
             Protected skills (spec, run, fix_signal) cannot be customised at the project level.\n\
             To propose a change, run `moeb spec` and target the source at\n\
             src/moeb/internal/skills/{name}.skill.md."
        ));
    }
    if let Ok(content) = std::fs::read_to_string(&local_path) {
        return Ok(strip_skill_frontmatter(&content));
    }

    let asset_key = format!("skills/{}.skill.md", name);
    if let Some(asset) = crate::assets::Internal::get(&asset_key) {
        if let Ok(content) = std::str::from_utf8(asset.data.as_ref()) {
            return Ok(strip_skill_frontmatter(content));
        }
    }

    eprintln!(
        "moeb: warning: skill '{}' not found in .moeb/skills/ or binary assets; \
         workflow section will be empty.",
        name
    );
    Ok(String::new())
}

/// Returns the `review:` flag for the named skill (true = review enabled, false = opt-out).
pub fn load_skill_review_flag(moeb_dir: &Path, name: &str) -> bool {
    let local_path = moeb_dir.join("skills").join(format!("{}.skill.md", name));
    if let Ok(content) = std::fs::read_to_string(&local_path) {
        return extract_review_flag(&content);
    }
    let asset_key = format!("skills/{}.skill.md", name);
    if let Some(asset) = crate::assets::Internal::get(&asset_key) {
        if let Ok(content) = std::str::from_utf8(asset.data.as_ref()) {
            return extract_review_flag(content);
        }
    }
    true
}

/// Extracts the value of the `skill:` key from a spec's YAML frontmatter.
/// Returns None if the field is absent or the frontmatter cannot be parsed.
pub fn extract_skill_name(spec_content: &str) -> Option<String> {
    let body = spec_content.strip_prefix("---\n")?;
    let end = body.find("\n---")?;
    let yaml_str = &body[..end];
    for line in yaml_str.lines() {
        if let Some(val) = line.strip_prefix("skill:") {
            let name = val.trim().to_string();
            if !name.is_empty() {
                return Some(name);
            }
        }
    }
    None
}

/// Resolves and returns the content of the named role file.
///
/// Resolution order:
///   1. {moeb_dir}/roles/{name}.role.md  (project-local override)
///   2. Binary-bundled asset roles/{name}.role.md
///   3. Empty string with a stderr warning
pub fn load_role(moeb_dir: &Path, name: &str) -> String {
    let local_path = moeb_dir.join("roles").join(format!("{}.role.md", name));
    if let Ok(content) = std::fs::read_to_string(&local_path) {
        return content;
    }

    let asset_key = format!("roles/{}.role.md", name);
    if let Some(asset) = crate::assets::Internal::get(&asset_key) {
        if let Ok(content) = std::str::from_utf8(asset.data.as_ref()) {
            return content.to_string();
        }
    }

    eprintln!(
        "moeb: warning: role '{}' not found in .moeb/roles/ or binary assets; \
         role section will be empty.",
        name
    );
    String::new()
}

/// Extracts the value of the `role:` key from a spec's YAML frontmatter.
/// Returns None if the field is absent or the frontmatter cannot be parsed.
pub fn extract_role_name(spec_content: &str) -> Option<String> {
    let body = spec_content.strip_prefix("---\n")?;
    let end = body.find("\n---")?;
    let yaml_str = &body[..end];
    for line in yaml_str.lines() {
        if let Some(val) = line.strip_prefix("role:") {
            let name = val.trim().to_string();
            if !name.is_empty() {
                return Some(name);
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_skill_name_returns_some_when_present() {
        let spec = "---\ndomain: moeb\nslug: test\nstatus: active\nskill: my-skill\n---\n# Title\n";
        assert_eq!(extract_skill_name(spec), Some("my-skill".to_string()));
    }

    #[test]
    fn extract_skill_name_returns_none_when_absent() {
        let spec = "---\ndomain: moeb\nslug: test\nstatus: active\n---\n# Title\n";
        assert_eq!(extract_skill_name(spec), None);
    }

    #[test]
    fn extract_skill_name_returns_none_on_invalid_yaml() {
        let spec = "---\n: : : invalid yaml\n---\n# Title\n";
        assert_eq!(extract_skill_name(spec), None);
    }

    #[test]
    fn extract_role_name_returns_some_when_present() {
        let spec = "---\ndomain: moeb\nslug: test\nstatus: active\nrole: my-role\n---\n# Title\n";
        assert_eq!(extract_role_name(spec), Some("my-role".to_string()));
    }

    #[test]
    fn extract_role_name_returns_none_when_absent() {
        let spec = "---\ndomain: moeb\nslug: test\nstatus: active\n---\n# Title\n";
        assert_eq!(extract_role_name(spec), None);
    }

    #[test]
    fn extract_review_flag_defaults_true_when_absent() {
        let content = "# Skill body without frontmatter";
        assert!(extract_review_flag(content));
    }

    #[test]
    fn extract_review_flag_returns_true_when_set() {
        let content = "---\nreview: true\n---\n# Skill body";
        assert!(extract_review_flag(content));
    }

    #[test]
    fn extract_review_flag_returns_false_when_disabled() {
        let content = "---\nreview: false\n---\n# Skill body";
        assert!(!extract_review_flag(content));
    }

    #[test]
    fn strip_skill_frontmatter_removes_block() {
        let content = "---\nreview: true\n---\n# Skill body\nMore content";
        assert_eq!(strip_skill_frontmatter(content), "# Skill body\nMore content");
    }

    #[test]
    fn strip_skill_frontmatter_noop_without_frontmatter() {
        let content = "# Skill body\nMore content";
        assert_eq!(strip_skill_frontmatter(content), content);
    }

    #[test]
    fn load_skill_rejects_protected_skill_override() {
        let dir = tempfile::tempdir().unwrap();
        let skills_dir = dir.path().join("skills");
        std::fs::create_dir_all(&skills_dir).unwrap();
        for name in crate::internal::constants::PROTECTED_SKILLS {
            let skill_file = skills_dir.join(format!("{}.skill.md", name));
            std::fs::write(&skill_file, "# override").unwrap();
            let result = load_skill(dir.path(), name);
            assert!(result.is_err(), "expected Err for protected skill '{}'", name);
            let msg = result.unwrap_err().to_string();
            assert!(msg.contains("protected baseline skill"), "error message missing expected text for '{}'", name);
        }
    }
}
