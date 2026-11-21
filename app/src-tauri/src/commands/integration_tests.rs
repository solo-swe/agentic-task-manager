//! Integration: a subscription to a real workspace's list, in a state folder on disk, through atm-lib's store watcher.
//! Here rather than in the crate's `tests/`: it reads `AppState`'s own fields and methods.

use super::*;
use atm_lib::Workspaces;
use serde_json::{Value, json};
use std::time::Duration;
use tauri::ipc::Channel;

#[test]
fn a_subscription_says_when_a_watched_folder_changes_and_ends_with_unsubscribe() {
    let dir = std::env::temp_dir().join(format!("atm-app-watch-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("workspaces")).expect("a folder");
    let state = AppState::new(Workspaces::at(&dir));
    let (sent, received) = std::sync::mpsc::channel();
    let channel = Channel::new(move |body| {
        let _ = sent.send(body);
        Ok(())
    });
    let message = Changed {
        workspace: "work".into(),
        project: Some("abcd1234".into()),
    };
    // The default workspace's: its list, in the state folder
    let workspace = state.workspaces.default_workspace();
    let id = state
        .watch("main", |c| workspace.subscribe(c), channel, message)
        .expect("watching");
    std::thread::sleep(Duration::from_millis(200));
    std::fs::write(dir.join("workspaces").join("default.json"), "{}").expect("a change");
    let body = received
        .recv_timeout(Duration::from_secs(10))
        .expect("told of the change");
    let tauri::ipc::InvokeResponseBody::Json(text) = body else {
        panic!("a JSON message");
    };
    assert_eq!(
        serde_json::from_str::<Value>(&text).expect("json"),
        json!({"workspace": "work", "project": "abcd1234"})
    );
    // Only its own window ends it
    assert!(!state.unwatch("window-1", id));
    assert!(state.unwatch("main", id) && !state.unwatch("main", id));
    std::fs::remove_dir_all(&dir).expect("clean up");
}
