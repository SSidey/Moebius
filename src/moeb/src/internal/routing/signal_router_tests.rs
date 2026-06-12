use super::*;

#[test]
fn test_routing_disabled_when_pem_empty() {
    // MOEB_GITHUB_APP_PRIVATE_KEY_PEM is empty in dev builds; routing must be disabled.
    assert!(MOEB_GITHUB_APP_PRIVATE_KEY_PEM.is_empty());
}
