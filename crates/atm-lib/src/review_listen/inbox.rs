//! What the agent hasn't read (ATM-708): the inbox, and the line every command says while there is some.

use super::{
    FileStore, HashMap, HashSet, Path, PathBuf, Project, Result, Status, Value, cursor_file,
    is_agent, look, read_cursor, reviews, save_cursor, starting_point,
};

/// What the agent has read (ATM-708): beside the watch's cursor, the same kind of cursor, moved by
/// [`inbox`] and by a watch with `--once` (which ends to be read), never by a watch that runs on, whose lines go to a
/// file the agent may not read. What is past the watch's cursor but not this one was told and not read.
pub(super) fn inbox_file(root: Option<&Path>, me: &str) -> PathBuf {
    cursor_file(root, me).with_extension("inbox")
}

/// How far before a cursor was written its look reads the reviews changed: a file's time and a review's own (to the
/// second) may disagree a little; what was seen already isn't told again.
pub(super) const CHANGED_SLACK_SECS: i64 = 5;

/// When `file` was last written, less [`CHANGED_SLACK_SECS`], as a review's `updated` is written.
pub(super) fn written_at(file: &Path) -> Option<String> {
    let secs = std::fs::metadata(file)
        .and_then(|m| m.modified())
        .ok()?
        .duration_since(std::time::UNIX_EPOCH)
        .ok()?
        .as_secs();
    Some(atm_common::time::format_iso(
        i64::try_from(secs).unwrap_or(i64::MAX) - CHANGED_SLACK_SECS,
    ))
}

/// A cursor, and the time the reviews changed after it are to be read from.
type Start = (HashSet<String>, HashMap<String, Status>, Option<String>);

/// Where the inbox starts: its own cursor; the first time, the watch's (what was told before the inbox existed counts
/// as read, once), else from now. With the time it was written: only the reviews changed since can hold what is unread.
pub(super) fn inbox_start(store: &FileStore, root: Option<&Path>, me: &str) -> Result<Start> {
    let file = inbox_file(root, me);
    if let Some(cursor) = read_cursor(&file) {
        return Ok((
            cursor.seen.into_iter().collect(),
            cursor.statuses,
            written_at(&file),
        ));
    }
    // Kept at once: what a watch tells after this is unread until the inbox is read
    let watch_file = cursor_file(root, me);
    let (mut seen, mut statuses, since) = if let Some(cursor) = read_cursor(&watch_file) {
        (
            cursor.seen.into_iter().collect(),
            cursor.statuses,
            written_at(&watch_file),
        )
    } else {
        let (seen, statuses) = starting_point(store, &file, None)?;
        (seen, statuses, written_at(&file))
    };
    save_cursor(&file, &mut seen, &mut statuses);
    Ok((seen, statuses, since))
}

/// What `me` hasn't read about its work (ATM-708): every person's comment and decision since it last read its inbox,
/// as a watch prints them, oldest first; with `read`, they are read from now on (`atm review inbox`, the hook);
/// without, only looked at (the line every command prints).
///
/// # Errors
/// The reviews can't be read.
pub fn inbox(project: &Project, me: &str, read: bool) -> Result<Vec<Value>> {
    let root = project.root().map(Path::to_path_buf);
    let store = reviews::store(project)?;
    let (mut seen, mut statuses, since) = inbox_start(&store, root.as_deref(), me)?;
    // Only the reviews changed since it was last read: cheap on every command while nothing changed
    let news = look(
        project,
        root.as_deref(),
        me,
        &store,
        Some(since.as_deref().unwrap_or("")),
        &mut seen,
        &mut statuses,
    )?;
    // Nothing unread: a look only moves past what is passed over for good, so it is kept too, and the next command
    // reads only what changes after this
    if read || news.is_empty() {
        save_cursor(&inbox_file(root.as_deref(), me), &mut seen, &mut statuses);
    }
    Ok(news)
}

/// What one event says in a line: `CR-34 approved`, `CR-35 comment by Hakan`.
pub(super) fn event_line(event: &Value) -> String {
    let review = event["review"].as_str().unwrap_or("a review");
    if let Some(decision) = event["decision"].as_str() {
        return format!("{review} {}", decision.replace('_', " "));
    }
    let kind = event["kind"].as_str().unwrap_or("comment");
    let author = event["author"].as_str().unwrap_or("someone");
    format!("{review} {kind} by {author}")
}

/// What every command an agent runs says on stderr while it has unread events about its work (ATM-708): how many, and
/// which, until it reads them. It doesn't depend on the agent remembering to read a watch's output. None for a person,
/// with nothing unread, or when the reviews can't be read (never in the command's way).
#[must_use]
pub fn unread_line(project: &Project, me: &str) -> Option<String> {
    if !is_agent(Some(me)) {
        return None;
    }
    unread_summary(&inbox(project, me, false).ok()?)
}

/// The unread line for `unread` events: the first three, and how many more; none without any.
pub(super) fn unread_summary(unread: &[Value]) -> Option<String> {
    if unread.is_empty() {
        return None;
    }
    let n = unread.len();
    let mut listed: Vec<String> = unread.iter().take(3).map(event_line).collect();
    if n > 3 {
        listed.push(format!("{} more", n - 3));
    }
    Some(format!(
        "{n} unread about your work: {}. `atm review inbox` shows them (and marks them read)",
        listed.join(", ")
    ))
}
