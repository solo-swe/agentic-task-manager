//! The project's settings in `meta` (the Python atm's `lib/settings.py` and `lib/code.py`'s workflow): the workflow
//! (direct or pull-request, docs/v4/skills-and-workflows.md §1) and the logo. Each change is one change of the store.

use std::path::{Component, Path, PathBuf};

use atm_common::vocabulary::{WORKFLOW_TYPES, workflow_of};
use atm_common::{Error, Result};
use atm_db::jsongit::logo::{Logo, find_logo, logo_file, normalize};
use atm_db::working::Working;
use serde_json::{Map, Value, json};

use crate::project::Project;

fn meta_of(document: &Value) -> Map<String, Value> {
    document
        .get("meta")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default()
}

fn meta_mut(document: &mut Value) -> &mut Map<String, Value> {
    if !document.get("meta").is_some_and(Value::is_object) {
        document["meta"] = json!({});
    }
    document["meta"]
        .as_object_mut()
        .expect("made an object just above")
}

/// `atm db init --meta-file`: merge `new`'s keys into `meta` (project, description, vocabularies, ...), creating the
/// task data when there is none, so `/atm:create-tasks` never writes the file itself. The vocabularies are checked
/// before anything is written; `meta.id` is kept. Returns (created, the keys set, sorted).
///
/// # Errors
/// Vocabularies that can't work, a store error.
pub fn init_meta(project: &mut Project, new: &Map<String, Value>) -> Result<(bool, Vec<String>)> {
    let new: Map<String, Value> = new
        .iter()
        .filter(|(k, _)| *k != "id")
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect();
    let mut keys: Vec<String> = new.keys().cloned().collect();
    keys.sort();
    if !project.store().exists() {
        let mut meta = Map::new();
        meta.insert(
            "created".into(),
            json!(&atm_common::time::format_iso(atm_common::time::now())[..10]),
        );
        meta.extend(new);
        let document = json!({"meta": meta, "tasks": []});
        // Refused, naming the problem, before anything is written
        atm_common::Vocabulary::of_document(&document)?;
        project.store_mut().create(&document)?;
        return Ok((true, keys));
    }
    project.store_mut().change(&mut |work: &mut Working| {
        let meta = meta_mut(&mut work.document);
        for (key, value) in &new {
            meta.insert(key.clone(), value.clone());
        }
        atm_common::Vocabulary::of_document(&work.document)?;
        Ok(())
    })?;
    Ok((false, keys))
}

/// Eight random hexadecimal digits: a new project's `meta.id`.
fn random_id() -> String {
    use std::hash::{BuildHasher, Hasher};
    let mut hasher = std::hash::RandomState::new().build_hasher();
    hasher.write_u128(
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_nanos()),
    );
    hasher.write_u32(std::process::id());
    format!("{:08x}", hasher.finish() & 0xffff_ffff)
}

/// `atm db init`: give the project a `meta.id` if it has none (the dashboard keeps each project's saved view under
/// it). The new ID, if it gave one.
///
/// # Errors
/// A store error.
pub fn ensure_project_id(project: &mut Project) -> Result<Option<String>> {
    if !project.store().exists() {
        return Ok(None);
    }
    let mut given = None;
    project.store_mut().change(&mut |work: &mut Working| {
        let meta = meta_mut(&mut work.document);
        if meta
            .get("id")
            .and_then(Value::as_str)
            .is_some_and(|id| !id.is_empty())
        {
            return Ok(());
        }
        let id = random_id();
        meta.insert("id".into(), json!(id));
        given = Some(id);
        Ok(())
    })?;
    Ok(given)
}

/// `atm project workflow`: the project's workflow with its defaults, or none when none is set.
///
/// # Errors
/// A task file that can't be read, a workflow atm can't follow.
pub fn get_workflow(project: &Project) -> Result<Option<Map<String, Value>>> {
    workflow_of(&meta_of(project.store().document()?))
}

/// What `atm project workflow set` changes besides the type; what isn't given stays as it is.
#[derive(Debug, Clone, Default)]
pub struct WorkflowChange {
    /// The branch PRs target, or that direct pushes go to.
    pub base: Option<String>,
    /// Run the project's verify before pushing.
    pub verify: Option<bool>,
    /// CI fix rounds before `/atm:push` stops.
    pub max_rounds: Option<i64>,
    /// The commit message's template.
    pub commit_template: Option<String>,
    /// `forbid`, `require` (with `trailer`) or `none` (removes the rule).
    pub trailers: Option<String>,
    /// The trailer lines `require` asks for.
    pub trailer: Vec<String>,
}

/// `atm project workflow set`: its type, and what is given of the rest. Returns the workflow with its defaults.
///
/// # Errors
/// An unknown type, `--trailers require` without lines, a workflow atm can't follow.
pub fn set_workflow(
    project: &mut Project,
    workflow_type: Option<&str>,
    change: &WorkflowChange,
) -> Result<Map<String, Value>> {
    let Some(workflow_type) = workflow_type.filter(|t| WORKFLOW_TYPES.contains(t)) else {
        return Err(Error::Invalid(format!(
            "atm project workflow set needs a type: {}",
            WORKFLOW_TYPES.join(" or ")
        )));
    };
    let mut changes = Map::new();
    changes.insert("type".into(), json!(workflow_type));
    if let Some(base) = &change.base {
        changes.insert("base".into(), json!(base));
    }
    if let Some(verify) = change.verify {
        changes.insert("verify".into(), json!(verify));
    }
    if let Some(rounds) = change.max_rounds {
        changes.insert("max_rounds".into(), json!(rounds));
    }
    if let Some(template) = &change.commit_template {
        changes.insert("commit_template".into(), json!(template));
    }
    match change.trailers.as_deref() {
        Some("forbid") => {
            changes.insert("trailers".into(), json!("forbid"));
        }
        Some("require") => {
            if change.trailer.is_empty() {
                return Err(Error::Invalid(
                    "--trailers require needs at least one --trailer LINE".into(),
                ));
            }
            changes.insert("trailers".into(), json!({"require": change.trailer}));
        }
        _ => {}
    }
    let remove_trailers = change.trailers.as_deref() == Some("none");
    let mut result = Map::new();
    project.store_mut().change(&mut |work: &mut Working| {
        let meta = meta_mut(&mut work.document);
        let mut workflow = meta
            .get("workflow")
            .and_then(Value::as_object)
            .cloned()
            .unwrap_or_default();
        for (key, value) in &changes {
            workflow.insert(key.clone(), value.clone());
        }
        if remove_trailers {
            workflow.shift_remove("trailers");
        }
        meta.insert("workflow".into(), Value::Object(workflow));
        // Checked before anything is written
        result = workflow_of(meta)?.unwrap_or_default();
        Ok(())
    })?;
    Ok(result)
}

/// The project's root, where its logo is looked for; none for a store without a working copy.
fn root_of(project: &Project) -> Option<PathBuf> {
    project.store().working_copy().map(Path::to_path_buf)
}

/// `atm project logo`: the logo the project has (set or found), and what `meta.logo` says.
///
/// # Errors
/// A task file that can't be read.
pub fn get_logo(project: &Project) -> Result<(Option<Logo>, Value)> {
    let meta = meta_of(project.store().document()?);
    let found = root_of(project).and_then(|root| find_logo(&root, &meta));
    Ok((found, meta.get("logo").cloned().unwrap_or(Value::Null)))
}

/// `path` relative to `base`, as Python's `relpath` gives it (both absolute; no links read).
fn relative_to(path: &Path, base: &Path) -> String {
    let (path, base): (Vec<Component>, Vec<Component>) =
        (path.components().collect(), base.components().collect());
    let common = path.iter().zip(&base).take_while(|(a, b)| a == b).count();
    let mut parts: Vec<String> = vec!["..".into(); base.len() - common];
    parts.extend(
        path[common..]
            .iter()
            .map(|c| c.as_os_str().to_string_lossy().into_owned()),
    );
    if parts.is_empty() {
        ".".into()
    } else {
        parts.join("/")
    }
}

/// `atm project logo set`: `meta.logo` becomes the image at `path` (absolute, or relative to `cwd`, where atm runs),
/// kept relative to the project's working copy. Returns the path kept.
///
/// # Errors
/// An image the dashboard can't be given (not an SVG, PNG, WebP or ICO, outside the project, missing, too large).
pub fn set_logo(project: &mut Project, path: &str, cwd: &Path) -> Result<String> {
    let relative = match root_of(project) {
        Some(root) => {
            let real_root = std::fs::canonicalize(&root).unwrap_or(root);
            let relative = relative_to(&normalize(&cwd.join(path)), &real_root);
            logo_file(&real_root, &relative)?;
            relative
        }
        None => path.to_string(),
    };
    project.store_mut().change(&mut |work: &mut Working| {
        meta_mut(&mut work.document).insert("logo".into(), json!(relative));
        Ok(())
    })?;
    Ok(relative)
}

/// `atm project logo unset`: remove `meta.logo`. Returns whether one was set.
///
/// # Errors
/// A store error.
pub fn unset_logo(project: &mut Project) -> Result<bool> {
    let mut removed = false;
    project.store_mut().change(&mut |work: &mut Working| {
        removed = meta_mut(&mut work.document).shift_remove("logo").is_some();
        Ok(())
    })?;
    Ok(removed)
}

#[cfg(test)]
mod tests {
    use super::relative_to;
    use std::path::Path;

    #[test]
    fn a_path_is_made_relative_as_relpath_does() {
        assert_eq!(
            relative_to(Path::new("/p/assets/logo.svg"), Path::new("/p")),
            "assets/logo.svg"
        );
        assert_eq!(
            relative_to(Path::new("/q/logo.svg"), Path::new("/p")),
            "../q/logo.svg"
        );
        assert_eq!(relative_to(Path::new("/p"), Path::new("/p")), ".");
    }
}
