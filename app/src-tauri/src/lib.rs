//! The atm app (T-455, docs/guides/tauri.md): the dashboard in a window of its own, on atm-lib as `atm serve` is.
//!
//! Its commands are in `commands`; tauri-specta writes their TypeScript types (`.generated/app/bindings.ts`, never
//! edited by hand). The window may call only the commands its capability lists (`capabilities/main.json`).

pub mod assets;
pub mod commands;
pub mod links;
pub mod menu;
pub mod updates;
pub mod windows;

use tauri::Manager;

use commands::{
    AppState, agents_status, claims, connect_agents, document, file, list_projects,
    list_workspaces, logo, project_tasks, search, subscribe_links, subscribe_project,
    subscribe_workspace, unsubscribe, version, worktrees,
};
use menu::{menu, menu_state, subscribe_menu};
use updates::{check_update, install_update};
use windows::{open_window, set_title, shell};

// What the app serves on its own `atm:` scheme (`assets`), embedded by build.rs
include!(concat!(env!("OUT_DIR"), "/atm_assets.rs"));

/// The app's commands, for Tauri and for the TypeScript types (each also in `build.rs`'s manifest and the main
/// window's capability).
#[must_use]
pub fn commands() -> tauri_specta::Builder<tauri::Wry> {
    tauri_specta::Builder::<tauri::Wry>::new().commands(tauri_specta::collect_commands![
        version,
        list_workspaces,
        list_projects,
        search,
        project_tasks,
        claims,
        worktrees,
        document,
        file,
        logo,
        subscribe_workspace::<tauri::Wry>,
        subscribe_project::<tauri::Wry>,
        unsubscribe::<tauri::Wry>,
        agents_status,
        connect_agents,
        subscribe_links::<tauri::Wry>,
        check_update,
        install_update,
        shell,
        set_title::<tauri::Wry>,
        open_window::<tauri::Wry>,
        subscribe_menu::<tauri::Wry>,
        menu::<tauri::Wry>,
        menu_state::<tauri::Wry>,
    ])
}

/// Start the app: one instance (opening it again focuses the window in front), the user's registry in its state, the
/// `atm://` links it is opened with sent to the page of the window in front (T-459, T-511).
///
/// # Panics
/// When Tauri can't start (no window system, a broken build).
pub fn run() {
    let commands = commands();
    // The single instance first, so a second instance stops before it sets anything up
    let first =
        tauri::Builder::default().plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            if let Some(window) = windows::front_or_new(app) {
                windows::focus(&window);
            }
        }));
    let app = with_plugins(first)
        .manage(AppState::new(atm_lib::Workspaces::open()))
        .invoke_handler(commands.invoke_handler())
        // A window's page loading again (a reload) ends that page's subscriptions (T-506, T-511): it can't say so
        // itself; other windows' stay
        .on_page_load(|webview, payload| {
            if payload.event() == tauri::webview::PageLoadEvent::Started {
                webview.state::<AppState>().unwatch_window(webview.label());
            }
        })
        .setup(move |app| {
            commands.mount_events(app);
            // The menus (T-512): macOS's menu bar; on Windows and Linux hidden, for the page to draw
            menu::refresh(app.handle());
            Ok(())
        })
        .on_menu_event(|app, event| menu::chosen(app, event.id().as_ref()))
        .build(context())
        .expect("the atm app");
    app.run(|app, event| match event {
        // The window in front is the one links, a second instance and the menu go to; a closed one's subscriptions end
        tauri::RunEvent::WindowEvent { label, event, .. } => {
            let state = app.state::<AppState>();
            match event {
                tauri::WindowEvent::Focused(true) => {
                    let changed = state.front().as_deref() != Some(label.as_str());
                    state.focused(&label);
                    if changed {
                        menu::refresh(app);
                    }
                }
                tauri::WindowEvent::Destroyed => {
                    state.window_closed(&label);
                }
                _ => {}
            }
        }
        // macOS hands the app the links it is opened with (the scheme is in Info.plist), started by one or running
        #[cfg(target_os = "macos")]
        tauri::RunEvent::Opened { urls } => {
            let Some(window) = windows::front_or_new(app) else {
                return;
            };
            let state = app.state::<AppState>();
            if urls
                .iter()
                .any(|url| state.links().open(window.label(), url.as_str()))
            {
                windows::focus(&window);
            }
        }
        _ => {}
    });
}

/// The plugins and schemes the app starts with, after the single instance: the same in the tests, which build the
/// app's context with them, so a plugin that can't start (its configuration, T-483) fails a test, not the app.
fn with_plugins<R: tauri::Runtime>(builder: tauri::Builder<R>) -> tauri::Builder<R> {
    builder
        // CompMD's sandbox frames on a scheme of their own, `compmd:`, so they keep their own policy and origin
        // (ADR-005); the plugin gives the page their `base`
        .plugin(tauri_plugin_compmd::init())
        // What atm ships for its page itself (CompMD's processor, its WebAssembly, the packs' modules) on `atm:`
        .register_uri_scheme_protocol(assets::SCHEME, |_context, request| {
            let response = assets::respond(ATM_ASSETS, request.method(), request.uri().path());
            if !response.status().is_success() {
                eprintln!(
                    "atm-app: {} {} {}",
                    response.status().as_u16(),
                    request.method(),
                    request.uri().path()
                );
            }
            response
        })
        // Its own updates (T-483), through the two commands in `updates`: the page has no permission of the plugin's.
        // The plugin reads `plugins.updater` in tauri.conf.json when it starts (the key, as `updates.rs` has it)
        .plugin(tauri_plugin_updater::Builder::new().build())
}

/// The app's context (`tauri.conf.json`, the capabilities, the dashboard's files), made once: it can be generated only
/// once in a crate.
fn context<R: tauri::Runtime>() -> tauri::Context<R> {
    tauri::generate_context!()
}

#[cfg(test)]
mod tests {
    use super::commands::{
        AppError, TaskFilters, claims_of, document_of, file_of, logo_of, projects_of, search_in,
        tasks_of, workspaces_of, worktrees_of,
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
    fn the_typescript_types_are_written_to_generated() {
        let path =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../.generated/app/bindings.ts");
        std::fs::create_dir_all(path.parent().expect("a folder")).expect("the generated folder");
        commands()
            .export(
                specta_typescript::Typescript::default()
                    .header("// Generated by the atm app's tests (app/src-tauri/src/lib.rs): never edit by hand"),
                &path,
            )
            .expect("the bindings");
        let text = std::fs::read_to_string(&path).expect("the bindings file");
        assert!(
            text.contains("listWorkspaces")
                && text.contains("subscribeProject")
                && text.contains("TaskFilters")
                && text.contains("AppError")
        );
    }

    /// I25 (T-461): the main window's capability allows exactly the app's own commands (the manifest in build.rs), one
    /// permission each, and no core or plugin API
    #[test]
    fn the_capability_allows_exactly_the_apps_commands() {
        let build = include_str!("../build.rs");
        let list = build
            .split_once(".commands(&[")
            .and_then(|(_, rest)| rest.split_once("])"))
            .map(|(list, _)| list)
            .expect("build.rs's command list");
        let mut commands: Vec<String> = list
            .split(',')
            .map(|c| c.trim().trim_matches('"').to_string())
            .filter(|c| !c.is_empty())
            .collect();
        commands.sort();
        let capability: Value = serde_json::from_str(include_str!("../capabilities/main.json"))
            .expect("capabilities/main.json");
        // The main window and the ones the app opens (T-511), nothing else
        assert_eq!(capability["windows"], json!(["main", "window-*"]));
        let mut allowed: Vec<String> = capability["permissions"]
            .as_array()
            .expect("permissions")
            .iter()
            .map(|p| p.as_str().expect("a permission").to_string())
            .collect();
        allowed.sort();
        let expected: Vec<String> = commands
            .iter()
            .map(|c| format!("allow-{}", c.replace('_', "-")))
            .collect();
        assert_eq!(allowed, expected);
        assert!(
            capability.get("remote").is_none(),
            "no remote URL may call commands"
        );
    }

    /// `CompMD`'s frames' origin in the app (ADR-004): its plugin's scheme, as macOS and Linux, and Windows and Android,
    /// name it
    const COMPMD_ORIGINS: &str = "compmd://localhost http://compmd.localhost";
    /// The folders the page loads scripts from and fetches (T-503): atm's own `CompMD` build on `atm:` (`assets`), and
    /// the frames' folder, whose Shiki and Temml a host-mode page loads; each in both forms
    const SCRIPT_FOLDERS: &str = "atm://localhost/assets/compmd/ http://atm.localhost/assets/compmd/ \
                                  compmd://localhost/frames/ http://compmd.localhost/frames/";

    /// I26 (T-461): the window's CSP is the dashboard server's (I13): no remote, inline or evaluated script, frames only
    /// `CompMD`'s, images only the app's own or `data:`, connections only to Tauri's IPC and `CompMD`'s files; no
    /// global Tauri object
    #[test]
    fn the_windows_csp_is_strict() {
        let conf: Value =
            serde_json::from_str(include_str!("../tauri.conf.json")).expect("tauri.conf.json");
        let csp = &conf["app"]["security"]["csp"];
        assert_eq!(csp["default-src"], json!("'none'"));
        // Scripts: only from the two folders, by path (ADR-004), which may compile WebAssembly; never blob:, data:
        // (CVE-2026-95626: they cancel Tauri's nonces) or 'self' (every file of the app's own protocol)
        assert_eq!(
            csp["script-src"],
            json!(format!("'wasm-unsafe-eval' {SCRIPT_FOLDERS}"))
        );
        assert_eq!(csp["frame-src"], json!(COMPMD_ORIGINS));
        assert_eq!(csp["img-src"], json!("'self' data:"));
        // Tauri's IPC, CompMD fetching its WebAssembly, the page fetching the packs' modules, the frames' libraries
        assert_eq!(
            csp["connect-src"],
            json!(format!("ipc: http://ipc.localhost {SCRIPT_FOLDERS}"))
        );
        // Back (T-503): CompMD's frames come from its own scheme, so the window's policy no longer reaches them
        assert_eq!(csp["frame-ancestors"], json!("'none'"));
        assert_eq!(csp["form-action"], json!("'none'"));
        assert_eq!(conf["app"]["security"]["freezePrototype"], json!(true));
        assert_eq!(conf["app"]["withGlobalTauri"], json!(false));
        // Tauri leaves style-src as written: its nonce there makes the browser ignore 'unsafe-inline' (T-502)
        assert_eq!(
            conf["app"]["security"]["dangerousDisableAssetCspModification"],
            json!(["style-src"])
        );
        // Every debug build (`cargo run`, `cargo tauri dev`) uses devCsp (T-502): this policy, plus the live-reload
        // socket of the CLI's own server, which `cargo tauri dev` serves the page from; CompMD's origin is the same in
        // every build (its plugin is in the app)
        let dev = &conf["app"]["security"]["devCsp"];
        for (directive, value) in csp.as_object().expect("the csp") {
            let value = value.as_str().expect("a directive");
            let expected = if directive == "connect-src" {
                format!("{value} ws://127.0.0.1:1430/__tauri_cli")
            } else {
                value.to_owned()
            };
            assert_eq!(dev[directive], json!(expected), "devCsp {directive}");
        }
        assert_eq!(
            dev.as_object().map(serde_json::Map::len),
            csp.as_object().map(serde_json::Map::len),
            "devCsp has the same directives"
        );
    }

    /// The app starts with its plugins and its configuration (T-483: the updater refused a `tauri.conf.json` with no
    /// `plugins.updater`, so the app stopped as it started, which no test built with the plugins saw)
    #[test]
    fn the_app_starts_with_its_plugins() {
        with_plugins(tauri::test::mock_builder())
            .build(context())
            .expect("the app with its plugins");
    }

    /// The state folder is named by atm-common's identifier, and the app's must be the same (ADR-003 §2)
    #[test]
    fn the_apps_identifier_is_atms() {
        let conf: serde_json::Value =
            serde_json::from_str(include_str!("../tauri.conf.json")).expect("tauri.conf.json");
        assert_eq!(conf["identifier"], atm_common::home::APP_ID);
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
        0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0x00, 0x00, 0x00, 0x0d, 0x49, 0x48, 0x44,
        0x52, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00, 0x00, 0x1f,
        0x15, 0xc4, 0x89, 0x00, 0x00, 0x00, 0x0a, 0x49, 0x44, 0x41, 0x54, 0x78, 0x9c, 0x63, 0x00,
        0x01, 0x00, 0x00, 0x05, 0x00, 0x01, 0x0d, 0x0a, 0x2d, 0xb4, 0x00, 0x00, 0x00, 0x00, 0x49,
        0x45, 0x4e, 0x44, 0xae, 0x42, 0x60, 0x82,
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
        let list = json!({"version": 1, "projects": [
            {"id": "abcd1234", "name": "App", "path": root.to_string_lossy()}
        ]});
        let folder = home.join("workspaces");
        std::fs::create_dir_all(&folder).expect("the workspaces folder");
        for w in ["default", "work"] {
            std::fs::write(folder.join(format!("{w}.json")), list.to_string())
                .expect("a workspace");
        }
        home
    }

    /// What the REST API answers for `path?query`: its status, and its body (as JSON, without `generated_at`, when
    /// it is JSON).
    fn rest(home: &Path, path: &str, query: &str) -> (u16, Value, Vec<u8>) {
        let projects = atm_api::routes::Projects::new(home);
        let response =
            atm_api::routes::serve(&projects, path, query, &axum::http::HeaderMap::new());
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
            document_of(&workspaces, "work", "abcd1234", "README.md").expect("the document")
        );
        let (status, _, bytes) = rest(&home, &format!("{p}/files"), "path=pixel.png");
        let image = file_of(&workspaces, "work", "abcd1234", "pixel.png").expect("the image");
        assert_eq!((status, bytes), (200, image.data));
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
        std::fs::remove_dir_all(&home).expect("clean up");
    }

    #[test]
    fn errors_keep_their_kind() {
        assert_eq!(
            AppError::from(atm_common::Error::failed("no")),
            AppError::Failed("no".into())
        );
        let json = serde_json::to_value(AppError::NotFound("T-1".into())).expect("json");
        assert_eq!(
            json,
            serde_json::json!({"kind": "not_found", "message": "T-1"})
        );
    }

    /// Each read command, its arguments, and what the plain function behind it answers (for a state folder made by
    /// `a_state_with_a_project`); `list_projects` last, as it opens `work`
    fn expected_answers(workspaces: &atm_lib::Workspaces) -> Vec<(&'static str, Value, Value)> {
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
                document_of(workspaces, "work", "abcd1234", "README.md").expect("document"),
            ),
            (
                "file",
                with(json!({"path": "pixel.png"})),
                serde_json::to_value(
                    file_of(workspaces, "work", "abcd1234", "pixel.png").expect("file"),
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
                claims,
                worktrees,
                document,
                file,
                logo,
                unsubscribe,
                agents_status,
                connect_agents,
            ])
            .build(context())
            .expect("the mock app");
        let window = tauri::WebviewWindowBuilder::new(&app, "main", tauri::WebviewUrl::default())
            .build()
            .expect("a window");
        let request = |command: &str, args: &Value, url: &str| tauri::webview::InvokeRequest {
            cmd: command.into(),
            callback: tauri::ipc::CallbackFn(0),
            error: tauri::ipc::CallbackFn(1),
            url: url.parse().expect("a URL"),
            body: tauri::ipc::InvokeBody::Json(args.clone()),
            headers: tauri::http::HeaderMap::default(),
            invoke_key: tauri::test::INVOKE_KEY.to_string(),
        };
        // Each command reaches its atm-lib call: it answers what the plain function does. `work` is opened first, as
        // list_projects (last) opens it
        workspaces.opened("work").expect("recent.json");
        for (command, args, expected) in expected_answers(&workspaces) {
            let answer = tauri::test::get_ipc_response(
                &window,
                request(command, &args, "tauri://localhost"),
            )
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

    /// ADR-004, T-503: the app serves what its page loads of `CompMD`'s build on `atm:`, at the paths `atm serve`
    /// gives them and with its headers, the packs' modules too; and no frame document, which `CompMD`'s plugin serves
    #[test]
    fn the_pages_compmd_files_come_from_atms_scheme_as_atm_serve_sends_them() {
        use tauri::http::Method;
        let compmd = std::fs::read_dir(
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../dashboard/dist/assets/compmd"),
        )
        .expect("the built dashboard's CompMD folder")
        .filter_map(Result::ok)
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .next()
        .expect("CompMD's version folder");
        // The headers CompMD needs from a host (H1, H2): the table `atm serve` is checked against too
        // (tests/scripts/serve.test.mjs), so the two hosts never drift apart
        let table: serde_json::Value = serde_json::from_str(include_str!(
            "../../../tests/scripts/compmd-host-headers.json"
        ))
        .expect("the host headers table");
        let files = table["files"].as_object().expect("files");
        let mut served = 0;
        for (file, headers) in files {
            let path = format!("/assets/compmd/{compmd}/{file}");
            let response = assets::respond(ATM_ASSETS, &Method::GET, &path);
            if assets::content_type(file).is_none() {
                assert_eq!(response.status(), 404, "{path}: a frame is the plugin's");
                continue;
            }
            served += 1;
            assert_eq!(response.status(), 200, "{path}");
            for (name, value) in headers.as_object().expect("headers") {
                let sent = response
                    .headers()
                    .get(name.as_str())
                    .and_then(|v| v.to_str().ok());
                assert_eq!(sent, value.as_str(), "{file}: {name}");
            }
        }
        assert!(served >= 2, "compmd.js and its WebAssembly");
        // The Mermaid pack's built module, which the page fetches and gives the sandbox as code
        let packs: Vec<&str> = ATM_ASSETS
            .iter()
            .map(|(path, _)| *path)
            .filter(|path| path.contains("/compmd/packs/"))
            .collect();
        assert!(
            packs.iter().any(|path| path.ends_with("/mermaid.js")),
            "{packs:?}"
        );
        assert!(
            ATM_ASSETS
                .iter()
                .all(|(path, _)| assets::content_type(path).is_some()),
            "only scripts and WebAssembly"
        );
    }

    /// T-502: the policy Tauri serves the dashboard's page with lets it work as under `atm serve`: its inline styles
    /// and `style` attributes apply (a nonce would make the browser ignore `'unsafe-inline'`), and `CompMD` loads and
    /// fetches its files from atm's scheme and its frames' folder
    #[test]
    fn the_page_is_served_with_a_policy_it_works_under() {
        let app = tauri::test::mock_builder()
            .build(context())
            .expect("the mock app");
        let page = app
            .asset_resolver()
            .get("tasks-dashboard.html".into())
            .expect("the dashboard's page in the app's files");
        let csp = page.csp_header.unwrap_or_default();
        let shown: String = csp
            .split_whitespace()
            .filter(|w| !w.starts_with("'sha256-"))
            .collect::<Vec<_>>()
            .join(" ");
        let directive = |name: &str| {
            csp.split(';')
                .map(str::trim)
                .find(|d| d.starts_with(&format!("{name} ")))
                .unwrap_or_default()
                .to_owned()
        };
        assert!(!directive("style-src").contains("'nonce-"), "{shown}");
        for name in ["script-src", "connect-src"] {
            for folder in [
                "atm://localhost/assets/compmd/",
                "compmd://localhost/frames/",
            ] {
                assert!(directive(name).contains(folder), "{name}: {shown}");
            }
        }
    }
}
