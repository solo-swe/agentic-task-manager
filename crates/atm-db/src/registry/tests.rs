use super::*;

fn meta(v: Value) -> Map<String, Value> {
    match v {
        Value::Object(m) => m,
        other => panic!("not an object: {other}"),
    }
}

#[test]
fn ids_are_meta_id_unless_taken_or_invalid_then_from_the_path() {
    let path = "/w/a";
    let hash = &sha1_hex(path.as_bytes())[..7];
    assert_eq!(project_id(&meta(json!({})), path), format!("p{hash}"));
    let registry = json!({"projects": [{"id": "x", "path": "/w/b"}]});
    assert_eq!(unique_id(&registry, path, "y"), "y");
    assert_eq!(unique_id(&registry, path, "x"), format!("x-{hash}"));
    assert_eq!(unique_id(&registry, path, "a b"), format!("p-{hash}"));
}

#[test]
fn an_entry_is_added_kept_and_removed_when_the_project_opts_out() {
    let mut registry = empty_registry();
    let path = Path::new("/w/proj");
    let m = meta(json!({"id": "proj", "project": "Proj"}));
    assert_eq!(update_entry(&mut registry, path, &m), "added");
    assert_eq!(update_entry(&mut registry, path, &m), "unchanged");
    assert_eq!(registry["projects"][0]["name"], json!("Proj"));
    let renamed = meta(json!({"id": "proj", "project": "New"}));
    assert_eq!(update_entry(&mut registry, path, &renamed), "updated");
    let out = meta(json!({"id": "proj", "dashboard": {"listed": false}}));
    assert_eq!(update_entry(&mut registry, path, &out), "removed");
    assert_eq!(update_entry(&mut registry, path, &out), "unlisted");
}
