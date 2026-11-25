//! The cache of other projects (T-553): fresh in one long-lived process, read again only when a project changed.
//! Each test has its own state folder and projects, set for its own thread only, so tests run side by side.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use atm_common::TASK_DIR;
use atm_db::MemoryStore;
use serde_json::{Value, json};

use super::*;
use crate::query::{Filters, task_views};

const API: &str = "0193a5b2-7c1d-7e2f-8a3b-4c5d6e7f8a01";
const POST: &str = "0193a5b2-7c1d-7e2f-8a3b-4c5d6e7f8b01";

/// A scratch folder with a state folder and a project B (`meta.id` `id`, key ATM) whose one task, ATM-1, has `status`,
/// registered in the `default` workspace; the lookups on this thread use that state folder.
struct Scene {
    dir: PathBuf,
    b: PathBuf,
    id: String,
}

impl Scene {
    fn new(name: &str) -> Self {
        let dir =
            std::env::temp_dir().join(format!("atm-lib-foreign-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let b = dir.join("b");
        std::fs::create_dir_all(b.join(TASK_DIR)).unwrap();
        std::fs::create_dir_all(dir.join("home/workspaces")).unwrap();
        let scene = Self {
            id: format!("{name}-b"),
            b,
            dir,
        };
        scene.write_b("in_progress");
        scene.register(true);
        TEST_HOME.with(|h| *h.borrow_mut() = Some(scene.dir.join("home")));
        TEST_FRESH_FOR.with(|f| f.set(None));
        install();
        scene
    }

    fn write_b(&self, status: &str) {
        let document = json!({
            "meta": {"id": self.id, "project": "atm", "format": 2, "key": "ATM", "next_number": 2},
            "tasks": [{"id": API, "number": 1, "key_final": true, "name": "the API", "status": status}],
        });
        std::fs::write(
            self.b.join(TASK_DIR).join("tasks.json"),
            serde_json::to_string_pretty(&document).unwrap(),
        )
        .unwrap();
    }

    fn register(&self, listed: bool) {
        let projects = if listed {
            json!([{"id": "b", "name": "atm", "path": self.b.to_string_lossy()}])
        } else {
            json!([])
        };
        std::fs::write(
            self.dir.join("home/workspaces/default.json"),
            json!({"version": 1, "projects": projects}).to_string(),
        )
        .unwrap();
    }

    fn reference(&self) -> String {
        format!("{}:{API}", self.id)
    }

    fn reads(&self) -> u64 {
        cache().projects.get(&self.b).map_or(0, |e| e.reads)
    }

    /// Project A, in memory: LFB-1 depends on ATM-1.
    fn a(&self) -> Project {
        let task = json!({"id": POST, "number": 1, "key_final": true, "name": "a post", "status": "todo",
            "dependencies": [self.reference()]});
        let meta = json!({"id": "a", "format": 2, "key": "LFB", "next_number": 2});
        let as_task = |v: Value| match v {
            Value::Object(map) => map,
            other => panic!("not a task: {other}"),
        };
        Project::new(Box::new(
            MemoryStore::new(vec![as_task(task)], Vec::new(), &as_task(meta)).unwrap(),
        ))
    }
}

impl Drop for Scene {
    fn drop(&mut self) {
        TEST_HOME.with(|h| *h.borrow_mut() = None);
        TEST_FRESH_FOR.with(|f| f.set(None));
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

fn derived(a: &Project) -> Value {
    task_views(a, &Filters::default()).unwrap()[0]["derived"].clone()
}

#[test]
fn a_change_in_the_other_project_is_seen_by_the_rules_and_the_display_without_a_restart() {
    let scene = Scene::new("done");
    let reference = scene.reference();
    let a = scene.a();
    assert_eq!(status(&reference), Some((false, json!("in_progress"))));
    let before = derived(&a);
    assert_eq!(before["ready"], json!(false));
    assert_eq!(before["waiting_for"], json!([reference]));
    assert_eq!(before["foreign"][&reference]["kind"], json!("active"));
    assert_eq!(label(&reference).as_deref(), Some("ATM-1"));

    scene.write_b("done");
    assert_eq!(status(&reference), Some((true, json!("done"))));
    let after = derived(&a);
    assert_eq!(after["ready"], json!(true), "{after}");
    assert_eq!(after["waiting_for"], json!([]));
    assert_eq!(after["foreign"][&reference]["status"], json!("done"));
    assert_eq!(after["foreign"][&reference]["kind"], json!("done"));
}

#[test]
fn an_unchanged_project_is_not_read_again_until_it_expires() {
    let scene = Scene::new("unchanged");
    let reference = scene.reference();
    // Through the registry first, as every use is (a project no registry lists is forgotten)
    assert!(status(&reference).is_some());
    let first = snapshot(&scene.b).unwrap();
    for _ in 0..3 {
        assert!(status(&reference).is_some());
        assert_eq!(Lookup::default().view(&reference)["found"], json!(true));
    }
    assert_eq!(scene.reads(), 1, "read once while its stamp is unchanged");
    assert!(Arc::ptr_eq(&first, &snapshot(&scene.b).unwrap()));
    let version = version(std::slice::from_ref(&scene.id));

    // Expired: read again, though its data is the same, so what was worked out from it stands
    TEST_FRESH_FOR.with(|f| f.set(Some(Duration::ZERO)));
    assert!(status(&reference).is_some());
    assert!(scene.reads() > 1, "read again once expired");
    assert!(Arc::ptr_eq(&first, &snapshot(&scene.b).unwrap()));
    assert_eq!(super::version(std::slice::from_ref(&scene.id)), version);

    // Changed: a new version
    scene.write_b("done");
    assert_ne!(super::version(std::slice::from_ref(&scene.id)), version);
}

#[test]
fn a_project_removed_from_the_workspaces_or_gone_is_unregistered_on_the_next_use() {
    let scene = Scene::new("removed");
    let reference = scene.reference();
    assert!(status(&reference).is_some());
    scene.register(false);
    assert_eq!(status(&reference), None);
    assert_eq!(
        Lookup::default().view(&reference)["reason"],
        json!("unregistered")
    );

    scene.register(true);
    assert!(status(&reference).is_some());
    std::fs::remove_dir_all(&scene.b).unwrap();
    assert_eq!(status(&reference), None);
    assert_eq!(
        Lookup::default().view(&reference)["reason"],
        json!("unregistered")
    );
}

/// ATM-773: an inbox item's task in another registered project, read back as that project has it now, with its name
#[test]
fn an_inbox_item_s_task_in_another_project_is_read_as_that_project_has_it() {
    let scene = Scene::new("inbox");
    let item = json!({"id": "IN-1", "text": "x", "state": "injected", "tasks": ["ATM-1", "ATM-7"],
        "created_at": "2026-11-24T09:00:00Z", "created_by": "Hakan", "updated_at": "2026-11-24T09:00:00Z"});
    let meta = json!({"id": "a", "format": 2, "key": "LFB", "next_number": 1});
    let Value::Object(meta) = meta else {
        unreachable!()
    };
    let store = MemoryStore::new(Vec::new(), Vec::new(), &meta)
        .unwrap()
        .with_inbox(
            json!({"version": 1, "next": 2, "items": [item]}),
            atm_common::inbox::empty_inbox(),
        );
    let a = Project::new(Box::new(store));
    let states = |a: &Project| crate::inbox::show(a, "IN-1").unwrap()["task_states"].clone();
    assert_eq!(
        states(&a),
        json!([
            {"key": "ATM-1", "found": true, "project": "atm", "name": "the API", "status": "in_progress",
             "kind": "active", "archived": false},
            {"key": "ATM-7", "found": false},
        ])
    );
    // As it is there now
    scene.write_b("done");
    assert_eq!(states(&a)[0]["kind"], json!("done"));
}

/// ATM-772: another registered project, by its key (any case) or its ID; never the one asking; none or two of a key
/// refused, saying which there are
#[test]
fn a_registered_project_is_found_by_its_key_or_id_never_the_one_asking() {
    let scene = Scene::new("named");
    for name in ["ATM", "atm", " atm ", scene.id.as_str()] {
        assert_eq!(registered_project(name, None).unwrap(), scene.b, "{name}");
    }
    // The one asking isn't another project
    let found = registered_project("ATM", Some(&scene.b)).unwrap_err();
    assert!(matches!(found, Error::NotFound(_)), "{found}");
    // None of that name: the registered ones named
    let missing = registered_project("LFB", None).unwrap_err();
    assert!(matches!(missing, Error::NotFound(_)));
    assert!(missing.to_string().contains("registered: ATM"), "{missing}");
    // A second project of the same key: refused, by their IDs
    let c = scene.dir.join("c");
    std::fs::create_dir_all(c.join(TASK_DIR)).unwrap();
    std::fs::write(
        c.join(TASK_DIR).join("tasks.json"),
        json!({"meta": {"id": "c-id", "key": "ATM", "format": 2}, "tasks": []}).to_string(),
    )
    .unwrap();
    std::fs::write(
        scene.dir.join("home/workspaces/default.json"),
        json!({"version": 1, "projects": [
            {"id": "b", "name": "atm", "path": scene.b.to_string_lossy()},
            {"id": "c", "name": "other", "path": c.to_string_lossy()},
        ]})
        .to_string(),
    )
    .unwrap();
    let both = registered_project("ATM", None).unwrap_err();
    assert!(matches!(both, Error::Invalid(_)), "{both}");
    assert!(both.to_string().contains("c-id"), "{both}");
    // Named by its ID, either one
    assert_eq!(registered_project("c-id", None).unwrap(), c);
    // Nothing registered: says how to register one
    scene.register(false);
    let none = registered_project("ATM", None).unwrap_err();
    assert!(
        none.to_string().contains("atm workspace add-project"),
        "{none}"
    );
}

#[test]
fn the_registry_is_read_again_only_when_the_workspaces_change() {
    let scene = Scene::new("registry");
    let first = registry();
    assert!(Arc::ptr_eq(&first, &registry()));
    scene.register(false);
    let second = registry();
    assert!(!Arc::ptr_eq(&first, &second) && second.is_empty());
}
