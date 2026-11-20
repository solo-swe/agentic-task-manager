//! Whether a watch runs: its lease, the agent's watch state, the reminder (ATM-642).

use super::*;

#[test]
fn a_watch_lease_holds_until_its_time_and_a_missing_or_broken_one_never() {
    for (text, holds_now) in [
        (Some("1000"), true),
        (Some(" 1000\n"), true),
        (Some("999"), false),
        (Some("500"), false),
        (Some("soon"), false),
        (Some(""), false),
        (None, false),
    ] {
        assert_eq!(holds(text, 999), holds_now, "{text:?}");
    }
}

#[test]
fn a_watch_writes_its_lease_while_it_runs_and_removes_it_when_it_ends() {
    let dir = std::env::temp_dir().join(format!("atm-lease-{}", std::process::id()));
    let file = dir.join("w.live");
    {
        let lease = Lease::take(file.clone());
        let now = atm_common::time::real_now();
        let text = std::fs::read_to_string(&file).unwrap();
        assert!(holds(Some(&text), now));
        assert!(
            !holds(Some(&text), now + LEASE_SECS + 1),
            "it runs out unrenewed"
        );
        lease.renew();
        assert!(file.exists());
    }
    assert!(!file.exists());
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn open_reviews_are_the_agents_own_or_naming_its_tasks_not_yet_decided_and_changed_ones_after_its_last_watch()
 {
    let at = "2026-11-19T05:00:00Z";
    let headers = [
        header(4, Status::Waiting, "claude:me", &[], "2026-11-19T05:30:00Z"),
        header(
            2,
            Status::ChangesRequested,
            "Hakan",
            &["ATM-1"],
            "2026-11-19T04:00:00Z",
        ),
        header(3, Status::Live, "claude:me", &[], at),
        header(
            5,
            Status::Waiting,
            "claude:other",
            &["ATM-9"],
            "2026-11-19T06:00:00Z",
        ),
        header(
            6,
            Status::Approved,
            "claude:me",
            &["ATM-1"],
            "2026-11-19T06:00:00Z",
        ),
        header(7, Status::Closed, "claude:me", &[], "2026-11-19T06:00:00Z"),
        header(8, Status::Draft, "claude:me", &[], "2026-11-19T06:00:00Z"),
    ];
    let mine: HashSet<String> = ["ATM-1".to_string()].into();
    let (open, changed) = open_and_changed(&headers, &mine, "claude:me", Some(at));
    assert_eq!(open, ["CR-2", "CR-3", "CR-4"]);
    // Changed after the last watch only: CR-3 at that very time was told
    assert_eq!(changed, ["CR-4"]);
    // Before any watch, nothing counts as changed since one
    let (_, none) = open_and_changed(&headers, &mine, "claude:me", None);
    assert_eq!(none, Vec::<String>::new());
    let (theirs, _) = open_and_changed(&headers, &HashSet::new(), "claude:other", Some(at));
    assert_eq!(theirs, ["CR-5"]);
}

#[test]
fn the_reminder_is_said_only_without_a_watch_with_reviews_open_and_again_after_ten_minutes_or_a_change()
 {
    let state = |running: bool, open: &[&str], changed: &[&str]| WatchState {
        running,
        open: open.iter().map(|r| (*r).to_string()).collect(),
        since_last_watch: changed.iter().map(|r| (*r).to_string()).collect(),
    };
    let at = |at: i64, changed: &[&str]| Reminded {
        at,
        changed: changed.iter().map(|r| (*r).to_string()).collect(),
    };
    let now = 10_000;
    // A watch runs, or nothing of its is open: nothing said
    assert_eq!(reminder_for(&state(true, &["CR-1"], &[]), None, now), None);
    assert_eq!(reminder_for(&state(false, &[], &[]), None, now), None);
    // Open, never said: said, naming them
    let said = reminder_for(&state(false, &["CR-1", "CR-2"], &[]), None, now).unwrap();
    assert!(said.contains("(2 open: CR-1, CR-2)"), "{said}");
    assert!(said.contains("atm review watch --json"), "{said}");
    // Said a minute ago, nothing new: quiet; ten minutes on, said again
    let quiet = state(false, &["CR-1"], &["CR-1"]);
    assert_eq!(
        reminder_for(&quiet, Some(&at(now - 60, &["CR-1"])), now),
        None
    );
    assert!(reminder_for(&quiet, Some(&at(now - REMIND_EVERY_SECS, &["CR-1"])), now).is_some());
    // A review changed that the last one didn't name: said at once, naming it
    let new = reminder_for(
        &state(false, &["CR-1", "CR-2"], &["CR-2"]),
        Some(&at(now - 60, &[])),
        now,
    )
    .unwrap();
    assert!(new.contains("changed since your last watch: CR-2"), "{new}");
    // Many open: the first two and the last
    let many = reminder_for(
        &state(false, &["CR-1", "CR-2", "CR-3", "CR-9"], &[]),
        None,
        now,
    )
    .unwrap();
    assert!(many.contains("(4 open: CR-1, CR-2, … CR-9)"), "{many}");
}
