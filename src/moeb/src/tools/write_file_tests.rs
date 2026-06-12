fn is_signal_catalogue_path(rel: &str) -> bool {
    let n = rel.replace('\\', "/");
    n.starts_with(".moeb/signals/catalogue/") && n.ends_with(".signal.json")
}

fn routing_condition_is_active(project_id: Option<&str>) -> bool {
    project_id != Some("moeb")
}

#[test]
fn test_signal_path_detection_matches_catalogue() {
    let path = ".moeb/signals/catalogue/a1b2c3d4-e5f6-7890-abcd-ef1234567890.signal.json";
    assert!(is_signal_catalogue_path(path));
}

#[test]
fn test_signal_path_detection_rejects_non_signal() {
    let path = ".moeb/specifications/moeb/moeb.example.md";
    assert!(!is_signal_catalogue_path(path));
}

#[test]
fn test_moeb_project_bypasses_routing() {
    // When project_id is "moeb", routing condition must evaluate false.
    assert!(!routing_condition_is_active(Some("moeb")));
    // When project_id is absent or a different value, routing is active.
    assert!(routing_condition_is_active(None));
    assert!(routing_condition_is_active(Some("other-project")));
}
