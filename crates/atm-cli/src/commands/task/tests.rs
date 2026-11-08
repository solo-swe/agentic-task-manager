use atm_lib::tasks::{Dependency, TaskView};
use serde_json::{Value, json};

use super::render_task;

fn view(task: Value) -> TaskView {
    let Value::Object(task) = task else {
        panic!("not a task: {task}")
    };
    TaskView {
        task,
        archived: false,
        priority: "P2".into(),
        started_by: None,
        dependencies: Vec::new(),
        related: Vec::new(),
        duplicated_by: Vec::new(),
    }
}

#[test]
fn a_task_in_full_as_the_python_atm_shows_it() {
    let mut v = view(json!({
        "id": "T-1", "name": "One", "type": "feature", "status": "done", "labels": ["api"],
        "description": "First line.\nSecond line.", "started_at": "2026-05-01T08:00:00Z",
        "completed_at": "2026-05-01T08:30:00Z", "completed_by": "alice", "commit": "abc1234", "files": ["src/a.ts"],
        "notes": [{"at": "2026-05-01T08:30:00Z", "by": "alice", "kind": "outcome", "text": "Shipped"}]
    }));
    v.priority = "P1".into();
    v.started_by = Some(json!("alice"));
    assert_eq!(
        render_task(&v),
        "T-1  One\n  done · backlog · feature · P1 · labels: api\n  files: src/a.ts\n  started 2026-05-01T08:00:00Z by \
         alice · completed 2026-05-01T08:30:00Z by alice\n  commit: abc1234\n  description:\n    First line.\n    \
         Second line.\n  notes:\n    2026-05-01T08:30:00Z outcome (alice): Shipped"
    );
}

#[test]
fn dependencies_resolution_and_relations() {
    let mut v = view(
        json!({"id": "T-2", "name": "Two", "type": "feature", "status": "skipped",
        "resolution": "duplicate", "duplicate_of": "T-1", "sprint": "S"}),
    );
    v.dependencies = vec![
        Dependency {
            id: "T-1".into(),
            status: Some(json!("done")),
            archived: false,
        },
        Dependency {
            id: "T-0".into(),
            status: Some(json!("done")),
            archived: true,
        },
        Dependency {
            id: "T-9".into(),
            status: None,
            archived: false,
        },
        Dependency {
            id: "T-8".into(),
            status: Some(Value::Null),
            archived: false,
        },
    ];
    v.related = vec!["T-4".into(), "T-1".into()];
    v.duplicated_by = vec!["T-5".into()];
    assert_eq!(
        render_task(&v),
        "T-2  Two\n  skipped · S · feature · P2\n  depends on: T-1 (done), T-0 (done, archived), T-9 (missing), T-8 \
         (missing)\n  resolution: duplicate (of T-1)\n  related: T-4, T-1\n  duplicated by: T-5"
    );
}

#[test]
fn an_archived_task_says_so_and_missing_fields_show_as_python_shows_them() {
    let mut v = view(json!({"id": "T-1"}));
    v.archived = true;
    assert_eq!(
        render_task(&v),
        "T-1  \n  None · backlog · archived · None · P2"
    );
}

#[test]
fn an_old_failure_shows_when_there_are_no_notes() {
    let v = view(
        json!({"id": "T-1", "name": "x", "status": "todo", "type": "bugfix",
        "last_failure": {"at": "2026-01-01T00:00:00Z", "reason": "tests failed"}}),
    );
    assert!(render_task(&v).ends_with("\n  last failure: 2026-01-01T00:00:00Z: tests failed"));
}

#[test]
fn a_text_where_a_list_belongs_is_joined_by_its_characters_as_python_does() {
    let v =
        view(json!({"id": "T-1", "name": "x", "status": "todo", "type": "docs", "files": "ab"}));
    assert!(render_task(&v).contains("\n  files: a, b"));
}
