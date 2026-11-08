//! The code side of tasks (the Python atm's `lib/code.py`): committing a task's files and recording the commit, the
//! pull request that contains tasks, its merge (docs/v4/atm-cli.md §4.5, data-model.md §4), and the repository's web
//! address. The commit goes through the project's version control ([`atm_vcs::GitVcs`]); tasks.json travels with
//! every code commit.

use atm_common::edits::touch;
use atm_common::py::{py_str, repr_str, strip};
use atm_common::repository::{move_pr_links, repository_url};
use atm_common::time::{now_iso, parse_iso};
use atm_common::vocabulary::workflow_of;
use atm_common::{Error, Kind, Result, TASK_DIR, TASKS_FILE, Task, Vocabulary};
use atm_db::working::Working;
use atm_vcs::GitVcs;
use serde_json::{Map, Value, json};

use crate::project::Project;

fn needs_git(vcs: Option<&GitVcs>) -> Result<&GitVcs> {
    vcs.filter(|v| v.in_repository())
        .ok_or_else(|| Error::failed("atm vcs commit needs a git repository"))
}

/// The position of the task with this ID in the change's document, or an error that says when it is archived.
fn task_at(work: &mut Working, task_id: &str) -> Result<usize> {
    let at = work.document["tasks"].as_array().and_then(|list| {
        list.iter()
            .position(|t| t.get("id").and_then(Value::as_str) == Some(task_id))
    });
    if let Some(at) = at {
        return Ok(at);
    }
    let archived = work.archive()?["tasks"].as_array().is_some_and(|list| {
        list.iter()
            .any(|t| t.get("id").and_then(Value::as_str) == Some(task_id))
    });
    let hint = if archived {
        " (it is archived: atm db unarchive brings it back)"
    } else {
        ""
    };
    Err(Error::failed(format!("no task {task_id}{hint}")))
}

/// Apply the workflow's trailer rule: `forbid` removes Co-Authored-By lines, `{require}` adds the missing lines.
fn commit_message(text: &str, trailers: Option<&Value>) -> String {
    match trailers {
        Some(Value::String(rule)) if rule == "forbid" => {
            let kept: Vec<&str> = text
                .lines()
                .filter(|line| !strip(line).to_lowercase().starts_with("co-authored-by:"))
                .collect();
            kept.join("\n").trim_end().to_string()
        }
        Some(Value::Object(rule)) => {
            let lines: Vec<&str> = text.lines().collect();
            let missing: Vec<String> = rule
                .get("require")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .map(py_str)
                .filter(|l| !lines.contains(&l.as_str()))
                .collect();
            if missing.is_empty() {
                text.to_string()
            } else {
                format!("{}\n\n{}", text.trim_end(), missing.join("\n"))
            }
        }
        _ => text.to_string(),
    }
}

/// The task's commit is `sha` (known only once the commit exists: a change for the next commit). Returns the task.
fn record_on_task(project: &mut Project, task_id: &str, sha: &str) -> Result<Task> {
    let actor = project.actor().map(String::from);
    let mut recorded = Task::new();
    project.store_mut().change(&mut |work: &mut Working| {
        let at = task_at(work, task_id)?;
        if let Some(task) = work.tasks()[at].as_object_mut() {
            task.insert("commit".into(), json!(sha));
            touch(task, actor.as_deref(), &now_iso());
            recorded.clone_from(task);
        }
        Ok(())
    })?;
    Ok(recorded)
}

/// The files named relative to `cwd`, relative to the repository instead (what [`commit_task`] takes), checked as a
/// commit checks them.
///
/// # Errors
/// No git repository; what [`GitVcs::paths`] refuses.
pub fn repository_paths(
    vcs: Option<&GitVcs>,
    files: &[String],
    cwd: &std::path::Path,
) -> Result<Vec<String>> {
    needs_git(vcs)?.paths(files, Some(cwd))
}

/// `atm vcs commit --dir` (T-405): every changed file under the folders, relative to the repository. Refused as a
/// whole when one of them is a file another agent's claimed task lists in its `files` (the file, or a folder holding
/// it): another agent's unfinished work in the same tree.
///
/// # Errors
/// No git repository; what [`GitVcs::changes_under`] refuses; a file another agent's claimed task lists.
pub fn folder_changes(
    project: &Project,
    vcs: Option<&GitVcs>,
    task_id: &str,
    folders: &[String],
    cwd: &std::path::Path,
) -> Result<Vec<String>> {
    let changed = needs_git(vcs)?.changes_under(folders, Some(cwd))?;
    let store = project.store().read_claims();
    for claim in store
        .get("claims")
        .and_then(Value::as_object)
        .into_iter()
        .flat_map(|c| c.values())
    {
        let owner_task = claim
            .get("task")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let agent = claim.get("agent").and_then(Value::as_str);
        if !claim.is_object() || owner_task == task_id || agent == project.actor() {
            continue;
        }
        let other = project.store().get_task(owner_task)?;
        let listed: Vec<&str> = other
            .and_then(|t| t.get("files"))
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
            .collect();
        for path in &changed {
            let owner = listed.iter().find(|f| {
                let f = f.trim_end_matches('/');
                path == f || path.starts_with(&format!("{f}/"))
            });
            if let Some(owner) = owner {
                return Err(Error::failed(format!(
                    "--dir would commit {path}, which {owner_task} lists ({owner}), claimed by {}: name your own files with --file",
                    atm_common::py::py_str_opt(claim.get("agent"))
                )));
            }
        }
    }
    Ok(changed)
}

/// `atm vcs commit`: commit exactly `files` (relative to the repository) plus tasks.json, with the workflow's message,
/// then record the commit on the task. Returns `{task, commit, message, files}`.
///
/// # Errors
/// No git repository or no files, an unknown task, an empty message, what git refuses.
pub fn commit_task(
    project: &mut Project,
    vcs: Option<&GitVcs>,
    task_id: &str,
    files: &[String],
    message: Option<&str>,
) -> Result<Value> {
    let vcs = needs_git(vcs)?;
    if files.is_empty() {
        return Err(Error::Invalid(
            "atm vcs commit needs --file (once per file the task changed), --dir FOLDER, or --record REV".into(),
        ));
    }
    let store = project.store();
    let task = if let Some(task) = store.get_task(task_id)? {
        task.clone()
    } else {
        let hint = if store.get_archived_task(task_id)?.is_some() {
            " (it is archived: atm db unarchive brings it back)"
        } else {
            ""
        };
        return Err(Error::failed(format!("no task {task_id}{hint}")));
    };
    let meta = store
        .document()?
        .get("meta")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    let workflow = workflow_of(&meta)?;
    let template = workflow
        .as_ref()
        .and_then(|w| w.get("commit_template"))
        .map_or_else(|| "feat({id}): {name}".to_string(), py_str);
    let mut text = message.map_or(template, String::from);
    for key in ["id", "name", "phase", "type"] {
        let value = task.get(key).map(py_str).unwrap_or_default();
        text = text.replace(&format!("{{{key}}}"), &value);
    }
    let text = commit_message(&text, workflow.as_ref().and_then(|w| w.get("trailers")));
    if strip(&text).is_empty() {
        return Err(Error::Invalid("the commit message is empty".into()));
    }
    let mut picked = vcs.paths(files, None)?;
    let task_file = format!("{TASK_DIR}/{TASKS_FILE}");
    if !picked.contains(&task_file) {
        picked.push(task_file);
    }
    let sha = vcs.commit(&picked, &text)?;
    let task = record_on_task(project, task_id, &sha)?;
    Ok(json!({"task": task.get("id"), "commit": sha, "message": text, "files": picked}))
}

/// Whether `id` appears in `text` as a whole word (not inside a longer ID: no letter, digit, `_` or `-` on either
/// side).
fn mentions(text: &str, id: &str) -> bool {
    let part = |c: Option<char>| c.is_some_and(|c| c.is_alphanumeric() || c == '_' || c == '-');
    text.match_indices(id).any(|(at, _)| {
        !part(text[..at].chars().next_back()) && !part(text[at + id.len()..].chars().next())
    })
}

/// `atm vcs commit --record` (T-343): record an existing commit on the current branch (the one a rebase or a
/// cherry-pick made of the task's commit) without committing anything. Returns `{task, commit, recorded, summary,
/// hint}`.
///
/// # Errors
/// No git repository, an option given as the revision, no such commit, one not on the current branch.
pub fn record_commit(
    project: &mut Project,
    vcs: Option<&GitVcs>,
    task_id: &str,
    rev: &str,
) -> Result<Value> {
    let vcs = needs_git(vcs)?;
    if rev.starts_with('-') {
        return Err(Error::Invalid(format!(
            "--record takes a revision, not an option: {}",
            repr_str(rev)
        )));
    }
    let sha = vcs
        .resolve(rev)
        .ok_or_else(|| Error::failed(format!("no commit {rev} in this repository")))?;
    if !vcs.in_history(&sha) {
        return Err(Error::failed(format!(
            "{} is not on the current branch (an ancestor of HEAD)",
            &sha[..7.min(sha.len())]
        )));
    }
    let text = vcs.message(&sha)?;
    let task = record_on_task(project, task_id, &sha)?;
    let id = task.get("id").map(py_str).unwrap_or_default();
    let hint = (!mentions(&text, &id))
        .then(|| format!("its message doesn't mention {id}: is it the task's commit?"));
    let summary = text.lines().next().unwrap_or_default();
    Ok(json!({"task": id, "commit": sha, "recorded": true, "summary": summary, "hint": hint}))
}

/// An http:// or https:// address with no space in it (the dashboard links to it: never `javascript:` or `data:`,
/// docs/security-review.md F9).
fn http_url(url: &str) -> bool {
    let lower = url.to_lowercase();
    let rest = lower
        .strip_prefix("https://")
        .or_else(|| lower.strip_prefix("http://"));
    rest.is_some_and(|r| !r.is_empty() && !r.chars().any(char::is_whitespace))
}

/// `atm vcs pr`: the PR that contains the tasks (DM-4.3: they are then In PR). Returns the task IDs.
///
/// # Errors
/// A number that isn't positive, an address that isn't http(s), an unknown task.
pub fn record_pr(
    project: &mut Project,
    ids: &[String],
    number: i64,
    url: &str,
) -> Result<Vec<String>> {
    if number <= 0 {
        return Err(Error::Invalid(
            "--number must be a positive whole number".into(),
        ));
    }
    if !http_url(url) {
        return Err(Error::Invalid(format!(
            "--url must be an http:// or https:// address, not {}",
            repr_str(url)
        )));
    }
    let actor = project.actor().map(String::from);
    let mut recorded = Vec::new();
    project.store_mut().change(&mut |work: &mut Working| {
        let found = ids
            .iter()
            .map(|id| task_at(work, id))
            .collect::<Result<Vec<usize>>>()?;
        let stamp = now_iso();
        recorded.clear();
        for at in found {
            if let Some(task) = work.tasks()[at].as_object_mut() {
                task.insert("pr".into(), json!({"number": number, "url": url}));
                task.shift_remove("merged_at");
                touch(task, actor.as_deref(), &stamp);
                recorded.push(task.get("id").map(py_str).unwrap_or_default());
            }
        }
        Ok(())
    })?;
    Ok(recorded)
}

fn pr_number(task: &Value) -> Option<i64> {
    task.get("pr")
        .filter(|p| p.is_object())
        .and_then(|p| p.get("number"))
        .and_then(Value::as_i64)
}

/// Release the planned releases the merged PR completes (SR-3.7): every task of the release in tasks.json (its own
/// release, or its sprint's) is in this PR and merged; archived and skipped tasks don't hold it back. Returns the
/// released IDs and, for the others, the tasks still keeping them planned.
fn release_with_merge(
    document: &mut Value,
    merged: &[usize],
    pr: i64,
    at: &str,
) -> Result<(Vec<String>, Map<String, Value>)> {
    let vocab = Vocabulary::of_document(document)?;
    let tasks: Vec<Task> = document["tasks"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_object)
        .cloned()
        .collect();
    let all = document["tasks"].as_array().cloned().unwrap_or_default();
    let touched: Vec<Value> = merged
        .iter()
        .filter_map(|i| all[*i].as_object())
        .filter_map(|t| vocab.effective_release(t))
        .collect();
    let url = merged.iter().find_map(|i| {
        all[*i]
            .get("pr")
            .and_then(|p| p.get("url"))
            .filter(|u| u.is_string())
            .cloned()
    });
    let (mut released, mut pending) = (Vec::new(), Map::new());
    // Not `document["meta"]`: indexing a missing key for change would add a null meta
    let Some(entries) = document
        .get_mut("meta")
        .and_then(|m| m.get_mut("releases"))
        .and_then(Value::as_array_mut)
    else {
        return Ok((released, pending));
    };
    for release in entries.iter_mut().filter_map(Value::as_object_mut) {
        let Some(id) = release.get("id").cloned().filter(|id| touched.contains(id)) else {
            continue;
        };
        if release
            .get("state")
            .and_then(Value::as_str)
            .unwrap_or("planned")
            != "planned"
        {
            continue;
        }
        // Its tasks merged in any PR count (T-486): a release's sprint may reach the base branch in several
        let mine: Vec<&Task> = tasks
            .iter()
            .filter(|t| vocab.effective_release(t).as_ref() == Some(&id))
            .filter(|t| vocab.kind_of(t.get("status")) != Some(Kind::Skipped))
            .collect();
        let waiting: Vec<Value> = mine
            .iter()
            .filter(|t| !t.get("merged_at").is_some_and(atm_common::py::truthy))
            .map(|t| t.get("id").cloned().unwrap_or(Value::Null))
            .collect();
        if !waiting.is_empty() {
            pending.insert(py_str(&id), Value::Array(waiting));
            continue;
        }
        // Released when its last task was merged: this merge, or a later one recorded before it
        let last = mine
            .iter()
            .filter_map(|t| t.get("merged_at").and_then(Value::as_str))
            .filter_map(|m| parse_iso(m).map(|when| (when, m)))
            .max_by_key(|(when, _)| *when)
            .map_or(at, |(_, m)| m);
        release.insert("state".into(), json!("released"));
        let mut record = Map::new();
        record.insert("number".into(), json!(pr));
        if let Some(url) = &url {
            record.insert("url".into(), url.clone());
        }
        release.insert("pr".into(), Value::Object(record));
        release.insert("released_at".into(), json!(last));
        released.push(py_str(&id));
    }
    Ok((released, pending))
}

/// `atm vcs pushed`: PR `pr` was merged (at `at`, default now), on every task that records it (DM-4.3: they are then
/// Merged); the releases it completes are released. Returns `{pr, tasks, already, released, pending}`.
///
/// # Errors
/// A time that isn't one, no task in that PR.
pub fn record_merge(project: &mut Project, pr: i64, at: Option<&str>) -> Result<Value> {
    if let Some(at) = at
        && parse_iso(at).is_none()
    {
        return Err(Error::Invalid(format!(
            "--at needs a time like 2026-10-24T18:00:00Z, not {}",
            repr_str(at)
        )));
    }
    let actor = project.actor().map(String::from);
    let mut outcome = Value::Null;
    project.store_mut().change(&mut |work: &mut Working| {
        let in_pr: Vec<usize> = work.document["tasks"]
            .as_array()
            .into_iter()
            .flatten()
            .enumerate()
            .filter(|(_, t)| t.is_object() && pr_number(t) == Some(pr))
            .map(|(i, _)| i)
            .collect();
        if in_pr.is_empty() {
            return Err(Error::failed(format!("no task records PR #{pr} (atm vcs pr records it)")));
        }
        let stamp = now_iso();
        let when = at.map_or_else(|| stamp.clone(), String::from);
        let (mut changed, mut already) = (Vec::new(), Vec::new());
        for i in &in_pr {
            let Some(task) = work.tasks()[*i].as_object_mut() else {
                continue;
            };
            let id = task.get("id").cloned().unwrap_or(Value::Null);
            if task.get("merged_at").is_some_and(atm_common::py::truthy) {
                already.push(id);
            } else {
                task.insert("merged_at".into(), json!(when));
                touch(task, actor.as_deref(), &stamp);
                changed.push(id);
            }
        }
        let (released, pending) = release_with_merge(&mut work.document, &in_pr, pr, &when)?;
        outcome = json!({"pr": pr, "tasks": changed, "already": already, "released": released, "pending": pending});
        Ok(())
    })?;
    Ok(outcome)
}

/// `atm vcs repository`: `meta.repository`, if set.
///
/// # Errors
/// A task file that can't be read.
pub fn get_repository(project: &Project) -> Result<Option<String>> {
    Ok(project.store().document()?["meta"]
        .get("repository")
        .filter(|r| atm_common::py::truthy(r))
        .map(py_str))
}

/// `atm vcs repository URL [--from OLD]`: set `meta.repository` to the web address of `url` (an http(s) URL or a git
/// remote). When the repository moved (from `previous`, else the address set before), the tasks' PR links move with
/// it, in tasks.json and the archive (T-273). Returns `{repository, moved_links, previous}`.
///
/// # Errors
/// An address that isn't one.
pub fn set_repository(project: &mut Project, url: &str, previous: Option<&str>) -> Result<Value> {
    let address = repository_url(url).ok_or_else(|| {
        Error::Invalid(format!(
            "{} is not a repository address: give an http(s) URL or a git remote",
            repr_str(url)
        ))
    })?;
    let old = match previous {
        Some(previous) => Some(repository_url(previous).ok_or_else(|| {
            Error::Invalid(format!(
                "--from {} is not a repository address: give an http(s) URL or a git remote",
                repr_str(previous)
            ))
        })?),
        None => None,
    };
    let mut outcome = Value::Null;
    project.store_mut().change(&mut |work: &mut Working| {
        if !work.document.get("meta").is_some_and(Value::is_object) {
            work.document["meta"] = json!({});
        }
        let set_before = work.document["meta"]
            .get("repository")
            .map(py_str)
            .unwrap_or_default();
        let before = old.clone().or_else(|| repository_url(&set_before));
        work.document["meta"]["repository"] = json!(address);
        let mut moved = 0;
        if let Some(before) = before.as_deref().filter(|b| *b != address) {
            moved += move_pr_links(work.tasks(), before, &address);
            if let Some(tasks) = work.archive()?["tasks"].as_array_mut() {
                moved += move_pr_links(tasks, before, &address);
            }
        }
        outcome = json!({"repository": address, "moved_links": moved, "previous": before});
        Ok(())
    })?;
    Ok(outcome)
}

/// `atm db init`: fill `meta.repository` from the version control's remote when there is none. The address set, if
/// it set one.
///
/// # Errors
/// A store error.
pub fn ensure_repository(project: &mut Project, vcs: Option<&GitVcs>) -> Result<Option<String>> {
    if !project.store().exists() {
        return Ok(None);
    }
    let Some(url) = vcs
        .and_then(GitVcs::remote_url)
        .and_then(|remote| repository_url(&remote))
    else {
        return Ok(None);
    };
    let mut set = None;
    project.store_mut().change(&mut |work: &mut Working| {
        if !work.document.get("meta").is_some_and(Value::is_object) {
            work.document["meta"] = json!({});
        }
        if work.document["meta"]
            .get("repository")
            .is_some_and(atm_common::py::truthy)
        {
            return Ok(());
        }
        work.document["meta"]["repository"] = json!(url);
        set = Some(url.clone());
        Ok(())
    })?;
    Ok(set)
}

/// The remote's default branch (`origin/HEAD`), else `main`: the base of a workflow that names none.
#[must_use]
pub fn default_base(cwd: &std::path::Path) -> String {
    let root = atm_db::jsongit::paths::project_root(cwd);
    atm_db::git::git(
        &["symbolic-ref", "--short", "refs/remotes/origin/HEAD"],
        &root,
    )
    .and_then(|r| r.split_once('/').map(|(_, branch)| branch.to_string()))
    .unwrap_or_else(|| "main".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trailers_are_removed_or_added_by_the_workflow() {
        let text = "feat: x\n\nCo-Authored-By: A <a@x>\n";
        assert_eq!(commit_message(text, Some(&json!("forbid"))), "feat: x");
        let rule = json!({"require": ["Signed-off-by: me"]});
        assert_eq!(
            commit_message("feat: x", Some(&rule)),
            "feat: x\n\nSigned-off-by: me"
        );
        assert_eq!(
            commit_message("feat: x\n\nSigned-off-by: me", Some(&rule)),
            "feat: x\n\nSigned-off-by: me"
        );
    }

    #[test]
    fn a_task_id_is_mentioned_only_as_a_whole_word() {
        assert!(mentions("feat(T-1): x", "T-1"));
        assert!(!mentions("feat(T-12): x", "T-1"));
        assert!(!mentions("xT-1", "T-1"));
    }

    #[test]
    fn only_web_addresses_are_pr_links() {
        assert!(http_url("https://github.com/o/r/pull/1"));
        assert!(http_url("HTTP://x"));
        assert!(!http_url("javascript:alert(1)"));
        assert!(!http_url("https://a b"));
    }
}
