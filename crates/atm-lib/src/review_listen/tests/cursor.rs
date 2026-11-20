//! The watch's cursor: where it was, kept per project and agent.

use super::*;

#[test]
fn a_watch_cursor_is_read_back_as_written_and_a_missing_or_broken_one_is_none() {
    let dir = std::env::temp_dir().join(format!("atm-lib-watch-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let file = dir.join("watches").join("x.json");
    assert!(read_cursor(&file).is_none());
    let mut seen: HashSet<String> = ["r1/c2".to_string(), "r1/c1".to_string()].into();
    let mut statuses: HashMap<String, Status> = [("r1".to_string(), Status::Approved)].into();
    save_cursor(&file, &mut seen, &mut statuses);
    let back = read_cursor(&file).unwrap();
    assert_eq!(back.seen, ["r1/c1", "r1/c2"]);
    assert_eq!(back.statuses, statuses);
    std::fs::write(&file, "{ not json").unwrap();
    assert!(read_cursor(&file).is_none());
    let _ = std::fs::remove_dir_all(&dir);
}

// ATM-663 (R12): two watches of one agent each wrote their own cursor over the other's

#[test]
fn a_watch_cursor_is_merged_with_what_another_watch_kept_meanwhile() {
    let dir = std::env::temp_dir().join(format!("atm-lib-watch-merge-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let file = dir.join("watches").join("x.json");
    // The other watch kept r1/c1 told and r2 approved
    let mut other: HashSet<String> = ["r1/c1".to_string()].into();
    let mut its: HashMap<String, Status> = [("r2".to_string(), Status::Approved)].into();
    save_cursor(&file, &mut other, &mut its);
    // This one told r1/c2 and saw r1 sent back, and r2 still waiting
    let mut seen: HashSet<String> = ["r1/c2".to_string()].into();
    let mut statuses: HashMap<String, Status> = [
        ("r1".to_string(), Status::ChangesRequested),
        ("r2".to_string(), Status::Waiting),
    ]
    .into();
    save_cursor(&file, &mut seen, &mut statuses);
    // Both kept on disk and in memory; for a review both saw, this watch's own status
    let back = read_cursor(&file).unwrap();
    assert_eq!(back.seen, ["r1/c1", "r1/c2"]);
    assert_eq!(seen, ["r1/c1".to_string(), "r1/c2".to_string()].into());
    assert_eq!(back.statuses, statuses);
    assert_eq!(statuses["r2"], Status::Waiting);
    let _ = std::fs::remove_dir_all(&dir);
}

// ATM-663 (R6): a merge renumbers a comment whose number two clones both gave

#[test]
fn a_comment_is_known_by_its_author_time_and_text_not_its_number() {
    let review = review(Status::Waiting, &["T-1"]);
    let first = comment("c3", "Hakan Serce", "Why?", vec![]);
    let renumbered = comment("c7", "Hakan Serce", "Why?", vec![]);
    assert_eq!(
        seen_key(&review.header, &first),
        seen_key(&review.header, &renumbered)
    );
    let other = comment("c3", "Hakan Serce", "Why not?", vec![]);
    assert_ne!(
        seen_key(&review.header, &first),
        seen_key(&review.header, &other)
    );
    let by_someone_else = comment("c3", "Ana", "Why?", vec![]);
    assert_ne!(
        seen_key(&review.header, &first),
        seen_key(&review.header, &by_someone_else)
    );
    // The key an older watch wrote is its number's
    assert_eq!(
        old_seen_key(&review.header, &first),
        format!("{}/c3", review.header.id.0)
    );
}

#[test]
fn a_watch_cursor_is_one_file_per_project_and_agent() {
    let (a, b) = (Path::new("/p/one"), Path::new("/p/two"));
    assert_eq!(
        cursor_file(Some(a), "claude:A"),
        cursor_file(Some(a), "claude:A")
    );
    assert_ne!(
        cursor_file(Some(a), "claude:A"),
        cursor_file(Some(a), "claude:B")
    );
    assert_ne!(
        cursor_file(Some(a), "claude:A"),
        cursor_file(Some(b), "claude:A")
    );
}
