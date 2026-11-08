//! The app's commands (docs/guides/tauri.md): thin mappings to atm's functions, with no validation, queries or rules
//! of their own. Their arguments and results are serde types, their errors one tagged enum the UI matches on.
//! (In a module of their own: Tauri's command macros can't be defined at a library's root.)

use serde::Serialize;
use specta::Type;
use tauri::State;

/// What every command shares: the user's one registry of projects, opened when the app starts.
pub struct AppState {
    registry: atm_boot::Registry,
}

impl AppState {
    /// The app's state on the registry `registry`.
    #[must_use]
    pub fn new(registry: atm_boot::Registry) -> Self {
        Self { registry }
    }
}

/// A command's error, as the UI sees it: atm's error kind, with its message.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
#[serde(tag = "kind", content = "message", rename_all = "snake_case")]
pub enum AppError {
    /// No such task, sprint, release, component or project.
    NotFound(String),
    /// The request itself is wrong.
    Invalid(String),
    /// It exists, but atm won't hand it out.
    Refused(String),
    /// Larger than atm hands out.
    TooLarge(String),
    /// Anything else atm refuses or can't do.
    Failed(String),
}

impl From<atm_common::Error> for AppError {
    fn from(error: atm_common::Error) -> Self {
        use atm_common::Error as E;
        match error {
            E::NotFound(m) => Self::NotFound(m),
            E::Invalid(m) => Self::Invalid(m),
            E::Refused(m) => Self::Refused(m),
            E::TooLarge(m) => Self::TooLarge(m),
            E::Failed { message, .. } => Self::Failed(message),
        }
    }
}

/// A project in the user's registry, as the dashboard lists it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
pub struct ProjectEntry {
    /// Its ID in the dashboard's addresses.
    pub id: String,
    /// Its name.
    pub name: String,
    /// Its folder.
    pub path: String,
    /// No readable task file there now.
    pub missing: bool,
}

/// atm's version.
#[tauri::command]
#[specta::specta]
#[must_use]
pub fn version() -> String {
    atm_common::VERSION.to_string()
}

/// The projects in the user's registry (`atm project list`).
///
/// # Errors
/// The registry's lock or file.
#[tauri::command]
#[specta::specta]
pub async fn list_projects(state: State<'_, AppState>) -> Result<Vec<ProjectEntry>, AppError> {
    projects(&state.registry)
}

/// The registry's projects, as entries.
///
/// # Errors
/// The registry's lock or file.
pub fn projects(registry: &atm_boot::Registry) -> Result<Vec<ProjectEntry>, AppError> {
    let listed = registry.list_projects()?;
    let text = |entry: &serde_json::Value, key: &str| {
        entry
            .get(key)
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default()
            .to_string()
    };
    Ok(listed["projects"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|entry| ProjectEntry {
            id: text(entry, "id"),
            name: text(entry, "name"),
            path: text(entry, "path"),
            missing: entry.get("missing").and_then(serde_json::Value::as_bool) == Some(true),
        })
        .collect())
}
