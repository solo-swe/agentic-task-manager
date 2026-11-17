//! What the routes' unit tests share: a state folder whose workspaces list one project, and an answer read back.

use std::path::{Path, PathBuf};

use atm_lib::workspaces::Member;
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::response::Response;
use serde_json::{Value, json};

use super::Projects;

/// A 1×1 PNG.
pub(super) const PIXEL: [u8; 67] = [
    0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0x00, 0x00, 0x00, 0x0d, 0x49, 0x48, 0x44, 0x52,
    0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00, 0x00, 0x1f, 0x15, 0xc4,
    0x89, 0x00, 0x00, 0x00, 0x0a, 0x49, 0x44, 0x41, 0x54, 0x78, 0x9c, 0x63, 0x00, 0x01, 0x00, 0x00,
    0x05, 0x00, 0x01, 0x0d, 0x0a, 0x2d, 0xb4, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45, 0x4e, 0x44, 0xae,
    0x42, 0x60, 0x82,
];

/// The project's ID in every workspace of [`a_state_with_a_project`].
pub(super) const PROJECT: &str = "abcd1234";

fn git(root: &Path, args: &[&str]) {
    let done = std::process::Command::new("git")
        .args(args)
        .current_dir(root)
        .output()
        .expect("git");
    assert!(done.status.success(), "git {args:?}");
}

/// A scratch state folder, `name`'s own, whose `default` and `work` workspaces list one project, [`PROJECT`]: a git
/// repository with its own identity and one commit, holding a task, a document (`README.md`), an image (`pixel.png`)
/// and a logo. The project is the folder's `project/`.
pub(super) fn a_state_with_a_project(name: &str) -> PathBuf {
    let home = std::env::temp_dir().join(format!("atm-api-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&home);
    let root = home.join("project");
    let task_dir = root.join(atm_common::TASK_DIR);
    std::fs::create_dir_all(&task_dir).expect("the task folder");
    let tasks = json!({
        "meta": {"id": PROJECT, "project": "Routes", "logo": "logo.png"},
        "tasks": [{"id": "T-1", "name": "Find the needle", "status": "todo",
                   "created_at": "2026-11-17T10:00:00Z", "updated_at": "2026-11-17T10:00:00Z"}],
    });
    std::fs::write(task_dir.join("tasks.json"), tasks.to_string()).expect("tasks.json");
    std::fs::write(root.join("README.md"), "# Routes\n\nA needle.\n").expect("README.md");
    std::fs::write(root.join("pixel.png"), PIXEL).expect("pixel.png");
    std::fs::write(root.join("logo.png"), PIXEL).expect("logo.png");
    git(&root, &["init", "-q"]);
    git(&root, &["config", "user.name", "Ana"]);
    git(&root, &["config", "user.email", "ana@example.com"]);
    git(&root, &["add", "-A"]);
    git(&root, &["commit", "-q", "-m", "The project"]);
    let list = json!({"version": 1, "projects": [
        {"id": PROJECT, "name": "Routes", "path": root.to_string_lossy()}
    ]});
    let folder = home.join("workspaces");
    std::fs::create_dir_all(&folder).expect("the workspaces folder");
    for w in ["default", "work"] {
        std::fs::write(folder.join(format!("{w}.json")), list.to_string()).expect("a workspace");
    }
    home
}

/// The project of `home`'s `default` workspace.
pub(super) fn member(home: &Path) -> Member {
    Projects::new(home)
        .workspaces
        .default_workspace()
        .member(PROJECT)
        .expect("the project")
}

/// A request that has the version `tag` already.
pub(super) fn having(tag: &str) -> HeaderMap {
    let mut request = HeaderMap::new();
    request.insert(
        header::IF_NONE_MATCH,
        HeaderValue::from_str(tag).expect("a tag"),
    );
    request
}

/// An answer, read back.
pub(super) struct Answer {
    pub status: StatusCode,
    pub headers: HeaderMap,
    pub body: Vec<u8>,
}

impl Answer {
    /// The body as JSON, without `generated_at` (Null when it isn't JSON).
    pub fn json(&self) -> Value {
        let mut body: Value = serde_json::from_slice(&self.body).unwrap_or(Value::Null);
        if let Some(map) = body.as_object_mut() {
            map.remove("generated_at");
        }
        body
    }

    /// The header `name`, or an empty text.
    pub fn header(&self, name: &str) -> &str {
        self.headers
            .get(name)
            .and_then(|v| v.to_str().ok())
            .unwrap_or_default()
    }
}

/// `response`, read.
pub(super) fn answer(response: Response) -> Answer {
    let status = response.status();
    let headers = response.headers().clone();
    let body = tokio::runtime::Builder::new_current_thread()
        .build()
        .expect("a runtime")
        .block_on(axum::body::to_bytes(response.into_body(), usize::MAX))
        .expect("the body")
        .to_vec();
    Answer {
        status,
        headers,
        body,
    }
}
