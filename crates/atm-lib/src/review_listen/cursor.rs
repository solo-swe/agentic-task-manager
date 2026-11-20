//! Where an agent's watch was (ATM-624): what it told or passed over, and each review's status as it last saw it.

use super::{
    Comment, FileStore, HashMap, HashSet, Path, PathBuf, Result, Review, ReviewHeader, Status,
    sha256_hex,
};

/// Where an agent's watch was (ATM-624): every comment it told or passed over, and each review's status as it last saw
/// it. The next watch starts from there, so what came while none was watching is told, once: a harness may end a
/// watch at any time (Claude Code's after 5 minutes). In atm's state folder, a file per project and agent.
#[derive(Debug, Default, serde::Serialize, serde::Deserialize)]
pub(super) struct Cursor {
    pub(super) seen: Vec<String>,
    pub(super) statuses: HashMap<String, Status>,
}

/// The cursor's file for `me` watching the project at `root`.
pub(super) fn cursor_file(root: Option<&Path>, me: &str) -> PathBuf {
    let key = format!(
        "{}\u{0}{me}",
        root.map(|r| r.display().to_string()).unwrap_or_default()
    );
    atm_common::home::state_dir()
        .join("watches")
        .join(format!("{}.json", &sha256_hex(key.as_bytes())[..16]))
}

/// The last watch's cursor; none when there was none, or it can't be read (then the watch starts from now).
pub(super) fn read_cursor(file: &Path) -> Option<Cursor> {
    serde_json::from_str(&std::fs::read_to_string(file).ok()?).ok()
}

/// A comment as the cursor knows it (ATM-663, R6): its review, its author, its time and a hash of its text, never its
/// number, which a merge renumbers when two clones gave the same one (a comment renumbered isn't told again).
pub(super) fn seen_key(review: &ReviewHeader, comment: &Comment) -> String {
    let text = &sha256_hex(comment.body.as_bytes())[..16];
    format!("{}/{}/{}/{text}", review.id.0, comment.author.0, comment.at)
}

/// The key older watches wrote (`review/cN`): still read, so a cursor from before ATM-663 tells nothing again.
pub(super) fn old_seen_key(review: &ReviewHeader, comment: &Comment) -> String {
    format!("{}/{}", review.id.0, comment.id.0)
}

/// Keep where the watch is, merged with what the agent's other watches of the project kept meanwhile (ATM-663, R12:
/// each wrote its own over the others'), under the watches' lock; `seen` and `statuses` become the merged ones, so a
/// comment another watch told isn't told again. A cursor that can't be written only means the next watch starts from
/// now.
pub(super) fn save_cursor(
    file: &Path,
    seen: &mut HashSet<String>,
    statuses: &mut HashMap<String, Status>,
) {
    let Some(dir) = file.parent() else { return };
    let _lock = atm_db::jsongit::lock::lock(dir, Some(2.0));
    if let Some(theirs) = read_cursor(file) {
        seen.extend(theirs.seen);
        for (review, status) in theirs.statuses {
            statuses.entry(review).or_insert(status);
        }
    }
    let mut listed: Vec<String> = seen.iter().cloned().collect();
    listed.sort();
    let cursor = Cursor {
        seen: listed,
        statuses: statuses.clone(),
    };
    if let Ok(text) = serde_json::to_string(&cursor) {
        let _ = std::fs::create_dir_all(dir);
        let _ = atm_common::home::write_text_atomic(file, &text);
    }
}

/// Every review the store holds, whole, in one read of the folder (ATM-708: a read per review read them all each
/// time, a hundred reviews ten thousand times a look).
pub(super) fn all_reviews(store: &FileStore) -> Result<Vec<Review>> {
    store.reviews()
}

/// Where a watch starts: the comments to pass over, and each review's status as last seen (a decision is a change to
/// a decided one, ATM-600). From the last watch's cursor (ATM-624); from `since`, what came after it is left to tell;
/// else from now, the cursor written so the next watch starts here.
pub(super) fn starting_point(
    store: &FileStore,
    file: &Path,
    since: Option<&str>,
) -> Result<(HashSet<String>, HashMap<String, Status>)> {
    if since.is_none()
        && let Some(cursor) = read_cursor(file)
    {
        return Ok((cursor.seen.into_iter().collect(), cursor.statuses));
    }
    let mut seen = HashSet::new();
    let mut statuses = HashMap::new();
    for review in all_reviews(store)? {
        seen.extend(
            review
                .comments()
                .filter(|c| since.is_none_or(|s| c.at.as_str() < s))
                .map(|c| seen_key(&review.header, c)),
        );
        // Decided since then: left out, so it is told
        if since.is_none_or(|s| review.header.updated.as_str() < s) {
            statuses.insert(review.header.id.0.clone(), review.header.status);
        }
    }
    if since.is_none() {
        save_cursor(file, &mut seen, &mut statuses);
    }
    Ok((seen, statuses))
}
