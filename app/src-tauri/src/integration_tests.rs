//! Integration: the commands on a real state folder (a project in a git repository, on disk), through atm-lib, and
//! through Tauri's mock runtime with the app's own context, so its capability decides what a window may call. (Here,
//! not in `tests/`: the commands' handler macros and the app's context are this crate's.)

use super::commands::{
    AppError, ReviewFilters, TaskFilters, claims_of, document_of, file_of, logo_of, projects_of,
    review_of, reviews_of, search_in, stats_of, tasks_of, workspaces_of, worktrees_of,
};
use super::*;
use serde_json::{Value, json};
use std::path::{Path, PathBuf};

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("atm-app-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("a scratch folder");
    dir
}

#[test]
fn an_empty_state_folder_has_only_default_with_no_projects() {
    let home = scratch("empty");
    let workspaces = atm_lib::Workspaces::at(&home);
    assert_eq!(
        workspaces_of(&workspaces),
        json!({"workspaces": [{"id": "default", "projects": 0}], "recent": "default"})
    );
    assert_eq!(
        projects_of(&workspaces, "default"),
        Ok(json!({"projects": []}))
    );
    assert_eq!(
        projects_of(&workspaces, "nope"),
        Err(AppError::NotFound("unknown workspace".into()))
    );
}

/// A 1×1 PNG.
const PIXEL: [u8; 67] = [
    0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0x00, 0x00, 0x00, 0x0d, 0x49, 0x48, 0x44, 0x52,
    0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00, 0x00, 0x1f, 0x15, 0xc4,
    0x89, 0x00, 0x00, 0x00, 0x0a, 0x49, 0x44, 0x41, 0x54, 0x78, 0x9c, 0x63, 0x00, 0x01, 0x00, 0x00,
    0x05, 0x00, 0x01, 0x0d, 0x0a, 0x2d, 0xb4, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45, 0x4e, 0x44, 0xae,
    0x42, 0x60, 0x82,
];

/// A state folder whose `default` and `work` workspaces list one project (a git repository with a task, a
/// document, an image and a logo).
fn a_state_with_a_project(name: &str) -> PathBuf {
    let home = scratch(name);
    let root = home.join("project");
    let task_dir = root.join(".agentic-task-manager");
    std::fs::create_dir_all(&task_dir).expect("the task folder");
    let tasks = json!({
        "meta": {"id": "abcd1234", "project": "App", "logo": "logo.png"},
        "tasks": [{"id": "T-1", "name": "Find the needle", "status": "todo",
                   "created_at": "2026-11-08T10:00:00Z", "updated_at": "2026-11-08T10:00:00Z"}],
    });
    std::fs::write(task_dir.join("tasks.json"), tasks.to_string()).expect("tasks.json");
    std::fs::write(root.join("README.md"), "# App\n\nA needle.\n").expect("README.md");
    std::fs::write(root.join("pixel.png"), PIXEL).expect("pixel.png");
    std::fs::write(root.join("logo.png"), PIXEL).expect("logo.png");
    let git = |args: &[&str]| {
        let done = std::process::Command::new("git")
            .args(args)
            .current_dir(&root)
            .output()
            .expect("git");
        assert!(done.status.success(), "git {args:?}");
    };
    git(&["init", "-q"]);
    git(&["add", "-A"]);
    // A commit, for a review's round to be made from
    git(&[
        "-c",
        "user.name=Tester",
        "-c",
        "user.email=tester@example.com",
        "-c",
        "commit.gpgsign=false",
        "commit",
        "-qm",
        "The project",
    ]);
    let list = json!({"version": 1, "projects": [
        {"id": "abcd1234", "name": "App", "path": root.to_string_lossy()}
    ]});
    let folder = home.join("workspaces");
    std::fs::create_dir_all(&folder).expect("the workspaces folder");
    for w in ["default", "work"] {
        std::fs::write(folder.join(format!("{w}.json")), list.to_string()).expect("a workspace");
    }
    home
}

/// The project's commit in a state from [`a_state_with_a_project`], by its full object ID: a page names a revision
/// so (ATM-671)
fn head_of(home: &Path) -> String {
    atm_lib::code::head_commit(&home.join("project")).expect("the project's commit")
}

/// What the REST API answers for `path?query`: its status, and its body (as JSON, without `generated_at`, when
/// it is JSON).
fn rest(home: &Path, path: &str, query: &str) -> (u16, Value, Vec<u8>) {
    let projects = atm_api::routes::Projects::new(home);
    let response = atm_api::routes::serve(&projects, path, query, &axum::http::HeaderMap::new());
    let status = response.status().as_u16();
    let bytes =
        tauri::async_runtime::block_on(axum::body::to_bytes(response.into_body(), usize::MAX))
            .expect("the body")
            .to_vec();
    let mut body: Value = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
    if let Some(map) = body.as_object_mut() {
        map.remove("generated_at");
    }
    (status, body, bytes)
}

/// ADR-003 §5: each read the app's commands make answers what its REST route does
#[test]
fn each_command_answers_what_its_rest_route_does() {
    let home = a_state_with_a_project("agree");
    let workspaces = atm_lib::Workspaces::at(&home);
    let v2 = "/api/v2/workspaces/work";
    let p = "/api/v2/workspaces/work/projects/abcd1234";
    let ok = |(status, body, _): (u16, Value, Vec<u8>)| {
        assert_eq!(status, 200, "{body}");
        body
    };

    assert_eq!(
        ok(rest(&home, "/api/v2/workspaces", "")),
        workspaces_of(&workspaces)
    );
    let listed = projects_of(&workspaces, "work").expect("the projects");
    assert_eq!(listed["projects"][0]["missing"], json!(false), "{listed}");
    assert_eq!(ok(rest(&home, &format!("{v2}/projects"), "")), listed);
    let found = search_in(&workspaces, "work", "needle").expect("the search");
    assert_eq!(found, json!({"abcd1234": ["T-1"]}));
    assert_eq!(ok(rest(&home, &format!("{v2}/search"), "q=needle")), found);
    let filters = TaskFilters {
        status: vec!["todo".into()],
        ..TaskFilters::default()
    };
    let tasks = tasks_of(&workspaces, "work", "abcd1234", filters).expect("the tasks");
    assert_eq!(ok(rest(&home, &format!("{p}/tasks"), "status=todo")), tasks);
    assert_eq!(
        ok(rest(&home, &format!("{p}/claims"), "")),
        claims_of(&workspaces, "work", "abcd1234").expect("the claims")
    );
    assert_eq!(
        ok(rest(&home, &format!("{p}/worktrees"), "")),
        worktrees_of(&workspaces, "work", "abcd1234").expect("the worktrees")
    );
    assert_eq!(
        ok(rest(&home, &format!("{p}/documents"), "path=README.md")),
        document_of(&workspaces, "work", "abcd1234", "README.md", None).expect("the document")
    );
    let listed =
        reviews_of(&workspaces, "work", "abcd1234", ReviewFilters::default()).expect("the reviews");
    assert_eq!(listed["reviews"], json!([]), "{listed}");
    assert_eq!(ok(rest(&home, &format!("{p}/reviews"), "")), listed);
    let (status, _, bytes) = rest(&home, &format!("{p}/files"), "path=pixel.png");
    let image = file_of(&workspaces, "work", "abcd1234", "pixel.png", None).expect("the image");
    assert_eq!((status, bytes), (200, image.data));
    // At a revision too (a review's image file's side, ATM-689), named by its full object ID (ATM-671)
    let head = head_of(&home);
    let (status, _, bytes) = rest(
        &home,
        &format!("{p}/files"),
        &format!("path=pixel.png&at={head}"),
    );
    let then = file_of(&workspaces, "work", "abcd1234", "pixel.png", Some(&head)).expect("at HEAD");
    assert_eq!((status, bytes), (200, then.data));
    // A name that moves is refused by both, the same way
    let (status, _, _) = rest(&home, &format!("{p}/files"), "path=pixel.png&at=HEAD");
    assert_eq!(status, 400);
    assert!(matches!(
        file_of(&workspaces, "work", "abcd1234", "pixel.png", Some("HEAD")),
        Err(AppError::Invalid(_))
    ));
    let (status, _, bytes) = rest(&home, &format!("{p}/logo"), "");
    let logo = logo_of(&workspaces, "work", "abcd1234")
        .expect("the logo")
        .expect("a logo");
    assert_eq!((status, bytes), (200, logo.data));

    // Refusals agree too
    assert_eq!(rest(&home, "/api/v2/workspaces/nope/projects", "").0, 404);
    assert_eq!(
        tasks_of(&workspaces, "work", "ffff0000", TaskFilters::default()),
        Err(AppError::NotFound("unknown project".into()))
    );
    assert_eq!(
        rest(&home, &format!("{v2}/projects/ffff0000/tasks"), "").0,
        404
    );
    assert!(matches!(
        review_of(&workspaces, "work", "abcd1234", "CR-9", None, None),
        Err(AppError::NotFound(_))
    ));
    assert_eq!(rest(&home, &format!("{p}/reviews/CR-9"), "").0, 404);
    assert!(matches!(
        reviews_of(
            &workspaces,
            "work",
            "abcd1234",
            ReviewFilters {
                status: vec!["merged".into()],
                ..ReviewFilters::default()
            }
        ),
        Err(AppError::Invalid(_))
    ));
    assert_eq!(rest(&home, &format!("{p}/reviews"), "status=merged").0, 400);
    std::fs::remove_dir_all(&home).expect("clean up");
}

/// Each read command, its arguments, and what the plain function behind it answers (for a state folder made by
/// `a_state_with_a_project`, its commit `head`); `list_projects` last, as it opens `work`
fn expected_answers(
    workspaces: &atm_lib::Workspaces,
    head: &str,
) -> Vec<(&'static str, Value, Value)> {
    let at = json!({"workspace": "work", "project": "abcd1234"});
    let with = |extra: Value| {
        let mut args = at.clone();
        if let (Some(args), Some(extra)) = (args.as_object_mut(), extra.as_object()) {
            args.extend(extra.clone());
        }
        args
    };
    vec![
        ("version", json!({}), json!(atm_common::VERSION)),
        ("shell", json!({}), json!(windows::shell_of())),
        ("list_workspaces", json!({}), workspaces_of(workspaces)),
        (
            "search",
            json!({"workspace": "work", "q": "needle"}),
            search_in(workspaces, "work", "needle").expect("search"),
        ),
        (
            "project_tasks",
            with(json!({"filters": {"status": ["todo"]}})),
            tasks_of(
                workspaces,
                "work",
                "abcd1234",
                TaskFilters {
                    status: vec!["todo".into()],
                    ..TaskFilters::default()
                },
            )
            .expect("tasks"),
        ),
        (
            "project_stats",
            with(json!({"tasks": ["T-1"], "from": null, "to": null})),
            stats_of(
                workspaces,
                "work",
                "abcd1234",
                Some(vec!["T-1".into()]),
                None,
                None,
            )
            .expect("stats"),
        ),
        (
            "claims",
            at.clone(),
            claims_of(workspaces, "work", "abcd1234").expect("claims"),
        ),
        (
            "worktrees",
            at.clone(),
            worktrees_of(workspaces, "work", "abcd1234").expect("worktrees"),
        ),
        (
            "document",
            with(json!({"path": "README.md"})),
            document_of(workspaces, "work", "abcd1234", "README.md", None).expect("document"),
        ),
        (
            "project_reviews",
            with(json!({"filters": {}})),
            reviews_of(workspaces, "work", "abcd1234", ReviewFilters::default()).expect("reviews"),
        ),
        (
            "file",
            with(json!({"path": "pixel.png"})),
            serde_json::to_value(
                file_of(workspaces, "work", "abcd1234", "pixel.png", None).expect("file"),
            )
            .expect("json"),
        ),
        (
            "file",
            with(json!({"path": "pixel.png", "at": head})),
            serde_json::to_value(
                file_of(workspaces, "work", "abcd1234", "pixel.png", Some(head))
                    .expect("file at HEAD"),
            )
            .expect("json"),
        ),
        (
            "logo",
            at.clone(),
            serde_json::to_value(logo_of(workspaces, "work", "abcd1234").expect("logo"))
                .expect("json"),
        ),
        ("unsubscribe", json!({"id": 99}), json!(false)),
        (
            "list_projects",
            json!({"workspace": "work"}),
            projects_of(workspaces, "work").expect("projects"),
        ),
    ]
}

/// Pages that may call no command: a remote one, one from `CompMD`'s own scheme, where its sandbox frames are
/// (ADR-004), and one from atm's asset scheme, which serves no page (T-503)
const ELSEWHERE: [&str; 5] = [
    "https://example.com",
    "compmd://localhost",
    "http://compmd.localhost",
    "atm://localhost",
    "http://atm.localhost",
];

/// ATM-677, audit A6: Tauri counts every scheme the app registers as local, `compmd:` among them, so the window's
/// capability alone lets a call from the `CompMD` frames through; the app's guard refuses it by its `Origin`, and still
/// answers the app's page
#[test]
#[cfg_attr(
    windows,
    ignore = "T-482: Windows' local origin differs (http://tauri.localhost?)"
)]
fn a_call_from_compmds_frames_is_refused_though_the_capability_covers_it() {
    let home = scratch("frames");
    let from = |origin: &str| {
        let mut request = ipc_request("version", &json!({}), origin);
        request.headers.insert(
            "origin",
            tauri::http::HeaderValue::from_str(origin).expect("a header"),
        );
        request
    };
    let serving_compmd = |builder: tauri::Builder<tauri::test::MockRuntime>| {
        builder
            .manage(AppState::new(atm_lib::Workspaces::at(&home)))
            .register_uri_scheme_protocol("compmd", |_context, _request| {
                tauri::http::Response::new(Vec::new())
            })
    };
    // Without the guard: the capability lets CompMD's frames call the app
    let open = serving_compmd(tauri::test::mock_builder())
        .invoke_handler(tauri::generate_handler![version])
        .build(context())
        .expect("the mock app");
    let window = tauri::WebviewWindowBuilder::new(&open, "main", tauri::WebviewUrl::default())
        .build()
        .expect("a window");
    assert!(
        tauri::test::get_ipc_response(&window, from("compmd://localhost")).is_ok(),
        "Tauri counts compmd: as local: without the guard a frame calls the app"
    );
    // With it, as the app runs: refused, and the page still answered
    let guarded = serving_compmd(tauri::test::mock_builder())
        .invoke_handler(navigation::guarded(tauri::generate_handler![version]))
        .build(context())
        .expect("the mock app");
    let window = tauri::WebviewWindowBuilder::new(&guarded, "main", tauri::WebviewUrl::default())
        .build()
        .expect("a window");
    let refused = tauri::test::get_ipc_response(&window, from("compmd://localhost"))
        .expect_err("a call from CompMD's frames");
    assert!(
        refused.to_string().contains("only the app's page"),
        "{refused}"
    );
    assert!(tauri::test::get_ipc_response(&window, from("tauri://localhost")).is_ok());
    std::fs::remove_dir_all(&home).expect("clean up");
}

/// Through the app's own context, so its capability is what lets the window call them
#[test]
#[cfg_attr(
    windows,
    ignore = "T-482: Windows' local origin differs (http://tauri.localhost?)"
)]
fn the_main_windows_capability_lets_it_call_the_commands() {
    let home = a_state_with_a_project("ipc");
    let workspaces = atm_lib::Workspaces::at(&home);
    let app = tauri::test::mock_builder()
        .manage(AppState::new(atm_lib::Workspaces::at(&home)))
        .invoke_handler(tauri::generate_handler![
            version,
            shell,
            list_workspaces,
            list_projects,
            search,
            project_tasks,
            project_stats,
            claims,
            worktrees,
            document,
            file,
            logo,
            project_reviews,
            project_review,
            unsubscribe,
            agents_status,
            connect_agents,
        ])
        .build(context())
        .expect("the mock app");
    let window = tauri::WebviewWindowBuilder::new(&app, "main", tauri::WebviewUrl::default())
        .build()
        .expect("a window");
    let request = ipc_request;
    // Each command reaches its atm-lib call: it answers what the plain function does. `work` is opened first, as
    // list_projects (last) opens it
    workspaces.opened("work").expect("recent.json");
    for (command, args, expected) in expected_answers(&workspaces, &head_of(&home)) {
        let answer =
            tauri::test::get_ipc_response(&window, request(command, &args, "tauri://localhost"))
                .expect(command);
        assert_eq!(
            answer.deserialize::<Value>().expect("json"),
            expected,
            "{command}"
        );
        // A page from anywhere else gets nothing (docs/guides/tauri.md)
        for elsewhere in ELSEWHERE {
            let refused =
                tauri::test::get_ipc_response(&window, request(command, &args, elsewhere));
            assert!(
                refused.is_err(),
                "{command} answered a page from {elsewhere}"
            );
        }
    }
    // An error is atm's kind, with its message
    let missing = tauri::test::get_ipc_response(
        &window,
        request(
            "claims",
            &json!({"workspace": "nope", "project": "x"}),
            "tauri://localhost",
        ),
    )
    .expect_err("no such workspace");
    assert_eq!(
        missing,
        json!({"kind": "not_found", "message": "unknown workspace"})
    );
    // Connecting agents (T-458): the status reads the harnesses atm knows; connecting reaches atm-lib's install,
    // which refuses a harness it doesn't know before writing anything (so this test writes nowhere), and a page
    // from anywhere else can't ask for it
    let status = tauri::test::get_ipc_response(
        &window,
        request("agents_status", &json!({}), "tauri://localhost"),
    )
    .expect("agents_status")
    .deserialize::<Value>()
    .expect("json");
    assert_eq!(status[0]["harness"], json!("claude"), "{status}");
    let unknown = tauri::test::get_ipc_response(
        &window,
        request(
            "connect_agents",
            &json!({"harness": "nope"}),
            "tauri://localhost",
        ),
    )
    .expect_err("an unknown harness");
    assert_eq!(unknown["kind"], json!("invalid"), "{unknown}");
    let remote = tauri::test::get_ipc_response(
        &window,
        request(
            "connect_agents",
            &json!({"harness": "claude"}),
            "https://example.com",
        ),
    );
    assert!(remote.is_err(), "connect_agents answered a remote page");
    std::fs::remove_dir_all(&home).expect("clean up");
}

/// A call of `command` with `args` from a page at `url`.
fn ipc_request(command: &str, args: &Value, url: &str) -> tauri::webview::InvokeRequest {
    tauri::webview::InvokeRequest {
        cmd: command.into(),
        callback: tauri::ipc::CallbackFn(0),
        error: tauri::ipc::CallbackFn(1),
        url: url.parse().expect("a URL"),
        body: tauri::ipc::InvokeBody::Json(args.clone()),
        headers: tauri::http::HeaderMap::default(),
        invoke_key: tauri::test::INVOKE_KEY.to_string(),
    }
}

/// The review commands (ATM-572) reach atm-lib's review requests, which refuse a body that isn't the request and
/// a review that isn't there before changing anything; a page from anywhere else can't call them
#[test]
#[cfg_attr(
    windows,
    ignore = "T-482: Windows' local origin differs (http://tauri.localhost?)"
)]
fn the_review_commands_reach_atm_lib_and_only_from_the_apps_page() {
    let home = a_state_with_a_project("review-ipc");
    let app = tauri::test::mock_builder()
        .manage(AppState::new(atm_lib::Workspaces::at(&home)))
        .invoke_handler(tauri::generate_handler![
            create_review,
            close_review,
            set_review_tasks,
            submit_review_decision,
            add_review_comment,
            update_review_comment,
            reply_to_review_comment,
        ])
        .build(context())
        .expect("the mock app");
    let window = tauri::WebviewWindowBuilder::new(&app, "main", tauri::WebviewUrl::default())
        .build()
        .expect("a window");
    let request = ipc_request;
    let on = |extra: Value| {
        let mut args = json!({"workspace": "work", "project": "abcd1234", "review": "CR-9"});
        args.as_object_mut()
            .expect("an object")
            .extend(extra.as_object().expect("an object").clone());
        args
    };
    let calls = [
        (
            "add_review_comment",
            on(json!({"comment": {"body": "Why?"}})),
        ),
        (
            "reply_to_review_comment",
            on(json!({"comment": "c1", "body": "Done."})),
        ),
        (
            "update_review_comment",
            on(json!({"comment": "c1", "update": {"resolved": true}})),
        ),
        (
            "submit_review_decision",
            on(json!({"decision": {"decision": "approve"}})),
        ),
        (
            "close_review",
            on(json!({"closing": {"status": "closed", "reason": "No."}})),
        ),
        ("set_review_tasks", on(json!({"tasks": []}))),
    ];
    for (command, args) in &calls {
        let missing =
            tauri::test::get_ipc_response(&window, request(command, args, "tauri://localhost"))
                .expect_err(command);
        assert_eq!(missing["kind"], json!("not_found"), "{command}: {missing}");
        for elsewhere in ELSEWHERE {
            assert!(
                tauri::test::get_ipc_response(&window, request(command, args, elsewhere)).is_err(),
                "{command} answered a page from {elsewhere}"
            );
        }
    }
    let not_one = tauri::test::get_ipc_response(
        &window,
        request(
            "add_review_comment",
            &on(json!({"comment": {"body": "x", "author": "claude:x"}})),
            "tauri://localhost",
        ),
    )
    .expect_err("not a comment");
    assert_eq!(not_one["kind"], json!("invalid"), "{not_one}");
    std::fs::remove_dir_all(&home).expect("clean up");
}

/// I25, T-511: a window the app opens (`window-<n>`) may call the commands as the main window does; a window by any
/// other label may call none
#[test]
#[cfg_attr(
    windows,
    ignore = "T-482: Windows' local origin differs (http://tauri.localhost?)"
)]
fn a_window_the_app_opens_has_the_main_windows_capability_and_no_other() {
    let home = scratch("windows");
    let app = tauri::test::mock_builder()
        .manage(AppState::new(atm_lib::Workspaces::at(&home)))
        .invoke_handler(tauri::generate_handler![
            version,
            shell,
            list_workspaces,
            unsubscribe
        ])
        .build(context())
        .expect("the mock app");
    let window = |label: &str| {
        tauri::WebviewWindowBuilder::new(&app, label, tauri::WebviewUrl::default())
            .build()
            .expect("a window")
    };
    let (opened, stranger) = (window("window-1"), window("other"));
    for (command, args) in [
        ("version", json!({})),
        ("shell", json!({})),
        ("list_workspaces", json!({})),
        ("unsubscribe", json!({"id": 1})),
    ] {
        let request = || tauri::webview::InvokeRequest {
            cmd: command.into(),
            callback: tauri::ipc::CallbackFn(0),
            error: tauri::ipc::CallbackFn(1),
            url: "tauri://localhost".parse().expect("a URL"),
            body: tauri::ipc::InvokeBody::Json(args.clone()),
            headers: tauri::http::HeaderMap::default(),
            invoke_key: tauri::test::INVOKE_KEY.to_string(),
        };
        assert!(
            tauri::test::get_ipc_response(&opened, request()).is_ok(),
            "{command} from window-1"
        );
        assert!(
            tauri::test::get_ipc_response(&stranger, request()).is_err(),
            "{command} answered a window the capability doesn't name"
        );
    }
    std::fs::remove_dir_all(&home).expect("clean up");
}

/// T-551: the window opens an external link only through the app's `open_url`, which refuses what isn't a web or
/// mail link with a typed error; the opener plugin's own commands are refused, as the page has no permission of
/// the plugin's. (Only refusals are asked for here: an accepted URL would open the browser.)
#[test]
#[cfg_attr(
    windows,
    ignore = "T-482: Windows' local origin differs (http://tauri.localhost?)"
)]
fn the_window_opens_links_only_through_open_url() {
    let home = scratch("open-url");
    let app = with_plugins(tauri::test::mock_builder())
        .manage(AppState::new(atm_lib::Workspaces::at(&home)))
        .invoke_handler(tauri::generate_handler![open_url])
        .build(context())
        .expect("the mock app");
    let window = tauri::WebviewWindowBuilder::new(&app, "main", tauri::WebviewUrl::default())
        .build()
        .expect("a window");
    let request = |command: &str, url: &str, from: &str| tauri::webview::InvokeRequest {
        cmd: command.into(),
        callback: tauri::ipc::CallbackFn(0),
        error: tauri::ipc::CallbackFn(1),
        url: from.parse().expect("a URL"),
        body: tauri::ipc::InvokeBody::Json(json!({ "url": url })),
        headers: tauri::http::HeaderMap::default(),
        invoke_key: tauri::test::INVOKE_KEY.to_string(),
    };
    for url in [
        "file:///etc/passwd",
        "atm://projects/abc",
        "javascript:alert(1)",
    ] {
        let refused =
            tauri::test::get_ipc_response(&window, request("open_url", url, "tauri://localhost"))
                .expect_err(url);
        assert_eq!(refused["kind"], json!("refused"), "{url}: {refused}");
    }
    let invalid = tauri::test::get_ipc_response(
        &window,
        request("open_url", "/projects/abc", "tauri://localhost"),
    )
    .expect_err("a path");
    assert_eq!(invalid["kind"], json!("invalid"), "{invalid}");
    // The plugin's own commands: no permission, from the window or anywhere else
    for from in std::iter::once("tauri://localhost").chain(ELSEWHERE) {
        for command in ["plugin:opener|open_url", "plugin:opener|open_path"] {
            let refused = tauri::test::get_ipc_response(
                &window,
                request(command, "https://example.invalid/", from),
            );
            assert!(refused.is_err(), "{command} answered a page from {from}");
        }
    }
    std::fs::remove_dir_all(&home).expect("clean up");
}

/// A mock app on `home`'s state with the app's context and `handler`, and its windows by label.
fn mock_app(
    home: &Path,
    handler: impl Fn(tauri::ipc::Invoke<tauri::test::MockRuntime>) -> bool + Send + Sync + 'static,
    labels: &[&str],
) -> (
    tauri::App<tauri::test::MockRuntime>,
    Vec<tauri::WebviewWindow<tauri::test::MockRuntime>>,
) {
    let app = tauri::test::mock_builder()
        .manage(AppState::new(atm_lib::Workspaces::at(home)))
        .invoke_handler(handler)
        .build(context())
        .expect("the mock app");
    let windows = labels
        .iter()
        .map(|label| {
            tauri::WebviewWindowBuilder::new(&app, *label, tauri::WebviewUrl::default())
                .build()
                .expect("a window")
        })
        .collect();
    (app, windows)
}

/// `command` with `args` from `window`'s page, the app's own: its answer, or its error.
fn call(
    window: &tauri::WebviewWindow<tauri::test::MockRuntime>,
    command: &str,
    args: &Value,
) -> Result<Value, Value> {
    tauri::test::get_ipc_response(window, ipc_request(command, args, "tauri://localhost"))
        .map(|answer| answer.deserialize::<Value>().expect("json"))
}

/// ATM-572: each review command answers what the page reads, after atm-lib did it: a review's header for those that
/// change the review, the review's and the comment's IDs for those on a comment (a reply's with the comment it
/// answers)
#[test]
#[cfg_attr(
    windows,
    ignore = "T-482: Windows' local origin differs (http://tauri.localhost?)"
)]
fn each_review_command_answers_what_the_page_reads() {
    let home = a_state_with_a_project("review-flow");
    let (_app, windows) = mock_app(
        &home,
        tauri::generate_handler![
            create_review,
            close_review,
            set_review_tasks,
            add_review_comment,
            update_review_comment,
            reply_to_review_comment,
            poke_review,
        ],
        &["main"],
    );
    let window = &windows[0];
    let on = |extra: Value| {
        let mut args = json!({"workspace": "work", "project": "abcd1234"});
        args.as_object_mut()
            .expect("an object")
            .extend(extra.as_object().expect("an object").clone());
        args
    };
    let opened = call(
        window,
        "create_review",
        &on(json!({"opening": {"title": "The README", "change": {"files": ["README.md"]}}})),
    )
    .expect("a review");
    assert_eq!(opened["reference"], json!("CR-1"), "{opened}");
    assert_eq!(opened["title"], json!("The README"), "{opened}");
    let review = |extra: Value| {
        let mut args = on(json!({"review": "CR-1"}));
        args.as_object_mut()
            .expect("an object")
            .extend(extra.as_object().expect("an object").clone());
        args
    };
    let added = call(
        window,
        "add_review_comment",
        &review(json!({"comment": {"body": "Why?"}})),
    )
    .expect("a comment");
    let comment = added["comment"].as_str().expect("its ID").to_owned();
    assert_eq!(added, json!({"review": "CR-1", "comment": comment}));
    let replied = call(
        window,
        "reply_to_review_comment",
        &review(json!({"comment": comment, "body": "Because."})),
    )
    .expect("a reply");
    let reply = replied["comment"].as_str().expect("its ID").to_owned();
    assert_ne!(reply, comment);
    assert_eq!(
        replied,
        json!({"review": "CR-1", "comment": reply, "reply_to": comment})
    );
    // ATM-611: a poke, as the user at the machine
    let poked = call(
        window,
        "poke_review",
        &review(json!({"body": "Where are we?"})),
    )
    .expect("a poke");
    assert_eq!(poked["review"], json!("CR-1"), "{poked}");
    assert!(poked["poke"].is_string(), "{poked}");
    assert_eq!(
        call(
            window,
            "update_review_comment",
            &review(json!({"comment": comment, "update": {"body": "Why so?"}})),
        ),
        Ok(json!({"review": "CR-1", "comment": comment}))
    );
    let tasks = call(
        window,
        "set_review_tasks",
        &review(json!({"tasks": ["T-1"]})),
    )
    .expect("its tasks");
    assert_eq!(tasks["reference"], json!("CR-1"), "{tasks}");
    assert_eq!(tasks["tasks"], json!(["T-1"]), "{tasks}");
    let closed = call(
        window,
        "close_review",
        &review(json!({"closing": {"status": "closed", "reason": "Not now."}})),
    )
    .expect("closed");
    assert_eq!(closed["status"], json!("closed"), "{closed}");
    // A body of the wrong shape is refused before atm-lib is asked
    let wrong = call(
        window,
        "create_review",
        &on(json!({"opening": {"title": "x", "colour": "red"}})),
    )
    .expect_err("not an opening");
    assert_eq!(wrong["kind"], json!("invalid"), "{wrong}");
    std::fs::remove_dir_all(&home).expect("clean up");
}

/// ATM-612: the user's drafts on a review, as atm-lib answers: listed (none here, so the test writes nothing to the
/// state folder, which it doesn't pick), and each change refused as atm-lib refuses it, before it keeps anything
#[test]
#[cfg_attr(
    windows,
    ignore = "T-482: Windows' local origin differs (http://tauri.localhost?)"
)]
fn the_draft_commands_answer_as_atm_lib_does() {
    let home = a_state_with_a_project("review-drafts");
    let (_app, windows) = mock_app(
        &home,
        tauri::generate_handler![
            create_review,
            list_review_drafts,
            add_review_draft,
            update_review_draft,
            remove_review_draft,
        ],
        &["main"],
    );
    let window = &windows[0];
    let on = |extra: Value| {
        let mut args = json!({"workspace": "work", "project": "abcd1234", "review": "CR-1"});
        args.as_object_mut()
            .expect("an object")
            .extend(extra.as_object().expect("an object").clone());
        args
    };
    call(
        window,
        "create_review",
        &on(json!({"opening": {"title": "The README", "change": {"files": ["README.md"]}}})),
    )
    .expect("a review");
    assert_eq!(
        call(window, "list_review_drafts", &on(json!({}))),
        Ok(json!({"review": "CR-1", "drafts": []}))
    );
    let empty = call(
        window,
        "add_review_draft",
        &on(json!({"draft": {"body": " "}})),
    )
    .expect_err("an empty draft");
    assert_eq!(empty["message"], json!("the draft is empty"), "{empty}");
    let shape = call(
        window,
        "add_review_draft",
        &on(json!({"draft": {"body": "x", "author": "claude:x"}})),
    )
    .expect_err("not a draft");
    assert_eq!(shape["kind"], json!("invalid"), "{shape}");
    for (command, args) in [
        ("update_review_draft", json!({"draft": "d9", "body": "x"})),
        ("remove_review_draft", json!({"draft": "d9"})),
    ] {
        let none = call(window, command, &on(args)).expect_err("no such draft");
        assert_eq!(
            none["message"],
            json!("you have no draft d9 on CR-1"),
            "{command}: {none}"
        );
    }
    std::fs::remove_dir_all(&home).expect("clean up");
}

/// T-518: a window's page saves its tabs (checked: none of the dashboard's pages is refused) and asks, once, for
/// those it comes back with
#[test]
#[cfg_attr(
    windows,
    ignore = "T-482: Windows' local origin differs (http://tauri.localhost?)"
)]
fn a_windows_page_saves_its_tabs_and_gets_back_those_it_comes_back_with() {
    let home = scratch("tabs");
    let (app, windows) = mock_app(
        &home,
        tauri::generate_handler![save_tabs, restored_tabs, reload_window],
        &["main", "window-1"],
    );
    let (main, other) = (&windows[0], &windows[1]);
    assert_eq!(call(main, "restored_tabs", &json!({})), Ok(Value::Null));
    let tabs = session::Tabs {
        addresses: vec!["/board".into(), "/projects/abc".into()],
        active: 1,
    };
    app.state::<AppState>()
        .session()
        .restore("main", tabs.clone());
    let back = json!({"addresses": ["/board", "/projects/abc"], "active": 1});
    assert_eq!(call(other, "restored_tabs", &json!({})), Ok(Value::Null));
    assert_eq!(call(main, "restored_tabs", &json!({})), Ok(back.clone()));
    assert_eq!(call(main, "restored_tabs", &json!({})), Ok(Value::Null));
    // Saving: only the dashboard's pages, kept for the window that saved them
    let refused = call(
        main,
        "save_tabs",
        &json!({"tabs": {"addresses": ["/api/v2/workspaces"], "active": 0}}),
    )
    .expect_err("no page of the dashboard's");
    assert_eq!(refused["kind"], json!("invalid"), "{refused}");
    assert_eq!(
        call(other, "save_tabs", &json!({"tabs": back})),
        Ok(Value::Null)
    );
    let saved = app.state::<AppState>().session().saved();
    assert_eq!(saved.len(), 1);
    assert_eq!(saved[0].tabs, tabs);
    // ATM-584: a Reload loads the window's page again, which gets back the tabs it showed, once
    assert_eq!(call(other, "reload_window", &json!({})), Ok(Value::Null));
    assert_eq!(call(other, "restored_tabs", &json!({})), Ok(back));
    assert_eq!(call(other, "restored_tabs", &json!({})), Ok(Value::Null));
    assert_eq!(
        call(main, "reload_window", &json!({})),
        Ok(Value::Null),
        "no tabs said yet"
    );
    assert_eq!(call(main, "restored_tabs", &json!({})), Ok(Value::Null));
    std::fs::remove_dir_all(&home).expect("clean up");
}

/// T-520: the app reads what needs the user from the workspace itself (the page names only the workspace), and
/// keeps what it saw for the next notification; an unknown workspace is its error
#[test]
#[cfg_attr(
    windows,
    ignore = "T-482: Windows' local origin differs (http://tauri.localhost?)"
)]
fn needs_you_reads_the_workspace_and_remembers_what_it_saw() {
    let home = a_state_with_a_project("needs-you");
    let (app, windows) = mock_app(&home, tauri::generate_handler![needs_you], &["main"]);
    let unknown = call(
        &windows[0],
        "needs_you",
        &json!({"workspace": "nope", "badge": true, "notify": true}),
    )
    .expect_err("no such workspace");
    assert_eq!(
        unknown,
        json!({"kind": "not_found", "message": "unknown workspace"})
    );
    for badge in [true, false] {
        assert_eq!(
            call(
                &windows[0],
                "needs_you",
                &json!({"workspace": "work", "badge": badge, "notify": false}),
            ),
            Ok(Value::Null)
        );
    }
    // It saw the project with nothing waiting: one task more is news
    let more = [badge::Waiting {
        id: "abcd1234".into(),
        name: "App".into(),
        count: 1,
    }];
    assert_eq!(
        app.state::<AppState>().needs_you().update("work", &more),
        ["App: 1 thing needs you"]
    );
    std::fs::remove_dir_all(&home).expect("clean up");
}

/// T-512: the Go menu's places are the workspaces and the shown workspace's projects, by name
#[test]
fn the_go_menus_places_are_the_workspaces_and_the_shown_ones_projects() {
    let home = a_state_with_a_project("places");
    let workspaces = atm_lib::Workspaces::at(&home);
    let ids = vec!["default".to_owned(), "work".to_owned()];
    assert_eq!(
        menu::places_of(&workspaces, Some("work")),
        menu::Places {
            workspaces: ids.clone(),
            projects: vec![("abcd1234".into(), "App".into())],
        }
    );
    assert_eq!(
        menu::places_of(&workspaces, Some("nope")),
        menu::Places {
            workspaces: ids,
            projects: vec![],
        }
    );
    std::fs::remove_dir_all(&home).expect("clean up");
}

/// T-512, T-513: each window's page reports its own menu state, and gets its own menus with it checked and its
/// workspace's projects in Go; a window not in front doesn't remake the menu bar
#[test]
#[cfg_attr(
    windows,
    ignore = "T-482: Windows' local origin differs (http://tauri.localhost?)"
)]
fn each_windows_menu_shows_what_its_page_reports() {
    let home = a_state_with_a_project("menus");
    let workspaces = atm_lib::Workspaces::at(&home);
    let (_app, windows) = mock_app(
        &home,
        tauri::generate_handler![menu, menu_state],
        &["main", "window-1"],
    );
    let (main, other) = (&windows[0], &windows[1]);
    // `main` is in front (nothing focused yet), so window-1's report changes no menu bar
    assert_eq!(
        call(
            other,
            "menu_state",
            &json!({"shown": {"view": "list", "workspace": "work"}}),
        ),
        Ok(Value::Null)
    );
    let shown = menu::MenuState {
        view: Some("list".into()),
        workspace: Some("work".into()),
        ..menu::MenuState::default()
    };
    let expected = |shown: &menu::MenuState| {
        serde_json::to_value(menu::definition(
            menu::Platform::current(),
            shown,
            &menu::places_of(&workspaces, shown.workspace.as_deref()),
        ))
        .expect("json")
    };
    assert_eq!(call(other, "menu", &json!({})), Ok(expected(&shown)));
    assert_eq!(
        call(main, "menu", &json!({})),
        Ok(expected(&menu::MenuState::default()))
    );
    std::fs::remove_dir_all(&home).expect("clean up");
}

/// T-513: the page's window buttons and title reach its own window; the title as the window can show it
#[test]
#[cfg_attr(
    windows,
    ignore = "T-482: Windows' local origin differs (http://tauri.localhost?)"
)]
fn the_window_commands_act_on_the_calling_window() {
    let home = scratch("window-commands");
    let (_app, windows) = mock_app(
        &home,
        tauri::generate_handler![
            set_title,
            minimize,
            toggle_maximize,
            start_dragging,
            close_window
        ],
        &["main"],
    );
    let missing = call(&windows[0], "set_title", &json!({})).expect_err("no title");
    assert!(missing.is_string(), "Tauri's own refusal: {missing}");
    // Closing last: the window goes
    for (command, args) in [
        ("set_title", json!({"title": "App\n(2)"})),
        ("minimize", json!({})),
        ("toggle_maximize", json!({})),
        ("start_dragging", json!({})),
        ("close_window", json!({})),
    ] {
        assert_eq!(
            call(&windows[0], command, &args),
            Ok(Value::Null),
            "{command}"
        );
    }
    std::fs::remove_dir_all(&home).expect("clean up");
}

/// ATM-761: the inbox's commands, as its REST routes: the items, an item added as the user, its text changed, a
/// person's decision; a body that isn't the request refused; a page from anywhere else can't call them
#[test]
#[cfg_attr(
    windows,
    ignore = "T-482: Windows' local origin differs (http://tauri.localhost?)"
)]
fn the_inbox_commands_answer_what_the_page_reads_and_only_from_the_apps_page() {
    let home = a_state_with_a_project("inbox-flow");
    let (_app, windows) = mock_app(
        &home,
        tauri::generate_handler![
            project_inbox,
            add_inbox_item,
            edit_inbox_item,
            decide_inbox_item
        ],
        &["main"],
    );
    let window = &windows[0];
    let on = |extra: Value| {
        let mut args = json!({"workspace": "work", "project": "abcd1234"});
        args.as_object_mut()
            .expect("an object")
            .extend(extra.as_object().expect("an object").clone());
        args
    };
    let list = |which: Value| {
        call(
            window,
            "project_inbox",
            &on(json!({"which": which, "archived": false})),
        )
    };
    assert_eq!(list(Value::Null).expect("the inbox")["items"], json!([]));
    let added = call(
        window,
        "add_inbox_item",
        &on(json!({"item": {"text": "A note"}})),
    )
    .expect("an item");
    assert_eq!(added["item"]["id"], json!("IN-1"), "{added}");
    // As the user at the machine: the inbox refuses an agent
    assert!(
        !added["item"]["created_by"]
            .as_str()
            .unwrap_or_default()
            .starts_with("claude:")
    );
    let edited = call(
        window,
        "edit_inbox_item",
        &on(json!({"id": "IN-1", "edit": {"text": "A better note"}})),
    )
    .expect("an edit");
    assert_eq!(edited["item"]["text"], json!("A better note"));
    let discarded = call(
        window,
        "decide_inbox_item",
        &on(json!({"id": "IN-1", "decision": {"decision": "discarded", "reason": "Done"}})),
    )
    .expect("a decision");
    assert_eq!(discarded["item"]["state"], json!("discarded"));
    assert_eq!(
        list(json!("discarded")).expect("the discarded")["discarded"],
        json!(1)
    );
    // Not a request: another author, an injection, a list
    for (command, args) in [
        (
            "add_inbox_item",
            on(json!({"item": {"text": "x", "created_by": "claude:x"}})),
        ),
        ("add_inbox_item", on(json!({"item": ["x"]}))),
        (
            "decide_inbox_item",
            on(json!({"id": "IN-1", "decision": {"decision": "injected", "tasks": ["T-1"]}})),
        ),
    ] {
        let refused = call(window, command, &args).expect_err(command);
        assert_eq!(refused["kind"], json!("invalid"), "{command}: {refused}");
    }
    // From anywhere but the app's own page: none answers
    for elsewhere in ELSEWHERE {
        assert!(
            tauri::test::get_ipc_response(
                window,
                ipc_request(
                    "add_inbox_item",
                    &on(json!({"item": {"text": "x"}})),
                    elsewhere
                )
            )
            .is_err(),
            "add_inbox_item answered a page from {elsewhere}"
        );
    }
    assert_eq!(
        list(json!("all")).expect("all")["items"]
            .as_array()
            .map(Vec::len),
        Some(1)
    );
    std::fs::remove_dir_all(&home).expect("clean up");
}
