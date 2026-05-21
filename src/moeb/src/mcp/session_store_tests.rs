use tempfile::TempDir;

use super::SessionStore;

#[test]
fn new_session_created_when_id_absent() {
    let dir = TempDir::new().unwrap();
    let store = SessionStore::new();
    let (id, is_new) = store.get_or_create(None, dir.path());
    assert!(is_new, "expected new session");
    assert!(!id.is_empty());
}

#[test]
fn existing_session_reused_when_id_present() {
    let dir = TempDir::new().unwrap();
    let store = SessionStore::new();
    let (id1, is_new1) = store.get_or_create(None, dir.path());
    assert!(is_new1);
    let (id2, is_new2) = store.get_or_create(Some(&id1), dir.path());
    assert!(!is_new2, "expected existing session");
    assert_eq!(id1, id2);
}
