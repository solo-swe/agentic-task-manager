//! Whether a watch runs (ATM-642): its lease, the agent's watch state, and the reminder when none runs.

use super::{
    HashSet, Path, PathBuf, Project, Result, ReviewFilter, ReviewHeader, ReviewStore, Value,
    cursor_file, is_agent, keyed_now, reviews,
};

/// How long a watch's lease lasts unrenewed (ATM-642). A watch renews it on every look, at least every [`RECHECK`], so
/// one that stopped, crashed or was killed is known to within this.
pub(super) const LEASE_SECS: i64 = 60;

/// How long atm stays quiet after reminding an agent that it isn't watching, unless something new came meanwhile.
pub(super) const REMIND_EVERY_SECS: i64 = 600;

/// The lease of this process's watch for `me` of the project at `root`, beside its cursor: one per watch (ATM-663,
/// R12: watches sharing one, the first to end removed it while the others ran).
pub(super) fn lease_file(root: Option<&Path>, me: &str) -> PathBuf {
    cursor_file(root, me).with_extension(format!("{}.live", std::process::id()))
}

/// Whether any watch of `me`'s on the project at `root` holds its lease at `now` (an older atm's one shared lease,
/// `<cursor>.live`, too). Leases that ran out are removed on the way: their watches were killed.
pub(super) fn any_lease_holds(root: Option<&Path>, me: &str, now: i64) -> bool {
    let cursor = cursor_file(root, me);
    let (Some(dir), Some(stem)) = (cursor.parent(), cursor.file_stem().and_then(|s| s.to_str()))
    else {
        return false;
    };
    let prefix = format!("{stem}.");
    let mut holding = false;
    for entry in std::fs::read_dir(dir).into_iter().flatten().flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        let lease = Path::new(&name).extension().is_some_and(|e| e == "live");
        if !(lease && name.starts_with(&prefix)) {
            continue;
        }
        if holds(std::fs::read_to_string(entry.path()).ok().as_deref(), now) {
            holding = true;
        } else {
            let _ = std::fs::remove_file(entry.path());
        }
    }
    holding
}

/// When atm last reminded `me` that it wasn't watching: beside its cursor.
pub(super) fn reminded_file(root: Option<&Path>, me: &str) -> PathBuf {
    cursor_file(root, me).with_extension("reminded")
}

/// A running watch's lease (ATM-642): until when it is known to run, in seconds since 1970, written while it runs
/// and removed when it ends. Each watch has its own ([`lease_file`]).
pub(super) struct Lease(PathBuf);

impl Lease {
    pub(super) fn take(file: PathBuf) -> Self {
        let lease = Self(file);
        lease.renew();
        lease
    }

    /// A lease that can't be written only means atm may remind the agent that it isn't watching.
    pub(super) fn renew(&self) {
        if let Some(dir) = self.0.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let until = atm_common::time::real_now() + LEASE_SECS;
        let _ = atm_common::home::write_private_atomic(&self.0, &until.to_string());
    }
}

impl Drop for Lease {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

/// Whether a lease reading `text` still holds at `now`.
pub(super) fn holds(text: Option<&str>, now: i64) -> bool {
    text.and_then(|t| t.trim().parse::<i64>().ok())
        .is_some_and(|until| until > now)
}

/// Whether an agent's reviews are watched (ATM-642): what `atm loop next --json` says as `watch`, and what atm reminds
/// an agent of when they aren't.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize)]
pub struct WatchState {
    /// A watch of the agent's runs now.
    pub running: bool,
    /// Without one: the reviews of its work not decided yet (its own, or naming a task it worked), what a watch would
    /// tell it about, by number.
    pub open: Vec<String>,
    /// Of those, the ones changed since its last watch stopped: a comment or a decision it hasn't been told. None
    /// before its first watch (that one starts from then).
    pub since_last_watch: Vec<String>,
}

/// The open reviews of `me`'s work among `headers`, and those of them changed after `last_watch` (ISO 8601).
pub(super) fn open_and_changed(
    headers: &[ReviewHeader],
    mine: &HashSet<String>,
    me: &str,
    last_watch: Option<&str>,
) -> (Vec<String>, Vec<String>) {
    let mut open: Vec<&ReviewHeader> = headers
        .iter()
        .filter(|h| h.status.in_play())
        .filter(|h| {
            h.author.as_ref().is_some_and(|a| a.0 == me) || h.tasks.iter().any(|t| mine.contains(t))
        })
        .collect();
    open.sort_by_key(|h| h.number);
    let changed = open
        .iter()
        .filter(|h| last_watch.is_some_and(|at| h.updated.as_str() > at))
        .map(|h| h.reference.0.clone())
        .collect();
    (
        open.iter().map(|h| h.reference.0.clone()).collect(),
        changed,
    )
}

/// Whether `me`'s reviews are watched now, and if not, what is open and what changed since its last watch. Cheap
/// enough for every command: the reviews' headers and the task list, no diffs.
///
/// # Errors
/// The reviews or the tasks can't be read.
pub fn watch_state(project: &Project, me: &str) -> Result<WatchState> {
    let root = project.root().map(Path::to_path_buf);
    if any_lease_holds(root.as_deref(), me, atm_common::time::real_now()) {
        return Ok(WatchState {
            running: true,
            ..WatchState::default()
        });
    }
    let store = reviews::store(project)?;
    let mut headers = store.list_reviews(&ReviewFilter::default())?;
    for header in &mut headers {
        keyed_now(project, &store, header);
    }
    let mine: HashSet<String> = project
        .store()
        .list_tasks()?
        .iter()
        .filter(|t| {
            ["started_by", "completed_by"]
                .iter()
                .any(|f| t.get(*f).and_then(Value::as_str) == Some(me))
        })
        .map(|t| project.task_key(t))
        .collect();
    // The last watch stopped when it last wrote its cursor
    let last_watch = std::fs::metadata(cursor_file(root.as_deref(), me))
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| atm_common::time::format_iso(i64::try_from(d.as_secs()).unwrap_or(i64::MAX)));
    let (open, since_last_watch) = open_and_changed(&headers, &mine, me, last_watch.as_deref());
    Ok(WatchState {
        running: false,
        open,
        since_last_watch,
    })
}

/// When atm last reminded the agent, and what had changed then.
#[derive(Debug, Default, serde::Serialize, serde::Deserialize)]
pub(super) struct Reminded {
    pub(super) at: i64,
    pub(super) changed: Vec<String>,
}

/// The reminder for `state` at `now`, after the one `last`: none while a watch runs or nothing of the agent's is
/// open; else once every [`REMIND_EVERY_SECS`], and at once when a review changed that the last one didn't name.
pub(super) fn reminder_for(
    state: &WatchState,
    last: Option<&Reminded>,
    now: i64,
) -> Option<String> {
    if state.running || state.open.is_empty() {
        return None;
    }
    let new = state
        .since_last_watch
        .iter()
        .any(|r| last.is_none_or(|l| !l.changed.contains(r)));
    if !new && last.is_some_and(|l| now - l.at < REMIND_EVERY_SECS) {
        return None;
    }
    let n = state.open.len();
    let open = if n <= 3 {
        state.open.join(", ")
    } else {
        format!("{}, … {}", state.open[..2].join(", "), state.open[n - 1])
    };
    let changed = if state.since_last_watch.is_empty() {
        String::new()
    } else {
        format!(
            "; changed since your last watch: {}",
            state.since_last_watch.join(", ")
        )
    };
    Some(format!(
        "you aren't watching your reviews ({n} open: {open}{changed}). Start `atm review watch --json --once` as \
         a background command, read what it prints when it ends, and start it again (atm docs reviews)"
    ))
}

/// What atm says on stderr when agent `me` runs a command with no watch of its reviews running and one of them open
/// (ATM-642), at most every ten minutes unless one changed; none otherwise. Never in the way of the command: a state
/// that can't be read says nothing.
#[must_use]
pub fn reminder(project: &Project, me: &str) -> Option<String> {
    if !is_agent(Some(me)) {
        return None;
    }
    let state = watch_state(project, me).ok()?;
    let file = reminded_file(project.root(), me);
    let last: Option<Reminded> = std::fs::read_to_string(&file)
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok());
    let now = atm_common::time::real_now();
    let line = reminder_for(&state, last.as_ref(), now)?;
    let reminded = Reminded {
        at: now,
        changed: state.since_last_watch,
    };
    if let (Some(dir), Ok(text)) = (file.parent(), serde_json::to_string(&reminded)) {
        let _ = std::fs::create_dir_all(dir);
        let _ = atm_common::home::write_private_atomic(&file, &text);
    }
    Some(line)
}
