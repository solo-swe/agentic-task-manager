use super::*;

/// A small seeded generator (xorshift), so the property tests are random but repeatable.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }

    fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }
}

const STATUSES: [&str; 4] = ["todo", "in_progress", "done", "blocked"];

fn random_task(rng: &mut Rng, id: u64) -> Value {
    let status = STATUSES[usize::try_from(rng.below(4)).unwrap()];
    let mut task = json!({
        "id": format!("T-{id:03}"),
        "name": format!("task {}", rng.below(5)),
        "status": status,
        "updated_at": format!("2026-01-0{}T00:00:00Z", 1 + rng.below(9)),
    });
    if rng.below(2) == 0 {
        task["notes"] = json!([{"at": format!("2026-01-0{}T00:00:00Z", 1 + rng.below(9)), "kind": "note", "text": "n"}]);
    }
    task
}

fn random_document(rng: &mut Rng) -> Value {
    let count = 1 + rng.below(5);
    let tasks: Vec<Value> = (1..=count).map(|id| random_task(rng, id)).collect();
    json!({"meta": {"project": "p"}, "tasks": tasks})
}

/// A changed copy: a task's name or status changed, one removed, or one added.
fn edit(rng: &mut Rng, document: &Value) -> Value {
    let mut changed = document.clone();
    let tasks = changed["tasks"].as_array_mut().unwrap();
    match rng.below(3) {
        0 if !tasks.is_empty() => {
            let at = usize::try_from(rng.below(tasks.len() as u64)).unwrap();
            tasks[at]["name"] = json!(format!("renamed {}", rng.below(100)));
        }
        1 if !tasks.is_empty() => {
            let at = usize::try_from(rng.below(tasks.len() as u64)).unwrap();
            tasks[at]["status"] = json!("done");
            tasks[at]["updated_at"] = json!("2026-02-01T00:00:00Z");
        }
        _ => {
            let id = 100 + rng.below(50);
            tasks.push(random_task(rng, id));
        }
    }
    changed
}

#[test]
fn merging_a_side_with_itself_or_an_unchanged_side_changes_nothing() {
    let mut rng = Rng(0x9E37_79B9_7F4A_7C15);
    for _ in 0..300 {
        let base = random_document(&mut rng);
        let ours = edit(&mut rng, &base);
        // Theirs didn't change: ours, as it is
        let (merged, conflicts) = merge_task_files(&base, &ours, &base);
        assert!(conflicts.is_empty(), "{conflicts:?}");
        assert_eq!(merged, ours);
        // Both sides the same: that version
        let (merged, conflicts) = merge_task_files(&base, &ours, &ours);
        assert!(conflicts.is_empty(), "{conflicts:?}");
        assert_eq!(merged, ours);
        // Ours didn't change: theirs' tasks (and no conflicts)
        let (merged, conflicts) = merge_task_files(&base, &base, &ours);
        assert!(conflicts.is_empty(), "{conflicts:?}");
        assert_eq!(merged["tasks"], ours["tasks"]);
    }
}

#[test]
fn every_task_either_side_added_is_kept_with_an_id_of_its_own() {
    let mut rng = Rng(42);
    for _ in 0..300 {
        let base = random_document(&mut rng);
        let (ours, theirs) = (edit(&mut rng, &base), edit(&mut rng, &base));
        let (merged, _) = merge_task_files(&base, &ours, &theirs);
        let ids: Vec<&str> = merged["tasks"]
            .as_array()
            .unwrap()
            .iter()
            .map(|t| t["id"].as_str().unwrap())
            .collect();
        let mut unique = ids.clone();
        unique.sort_unstable();
        unique.dedup();
        assert_eq!(unique.len(), ids.len(), "{ids:?}");
        let names = |d: &Value| -> Vec<String> {
            d["tasks"]
                .as_array()
                .unwrap()
                .iter()
                .map(|t| py_str(&t["name"]))
                .collect()
        };
        let added = |side: &Value| {
            side["tasks"].as_array().unwrap().len() > base["tasks"].as_array().unwrap().len()
        };
        if added(&ours) || added(&theirs) {
            assert!(names(&merged).len() >= names(&base).len() - 1);
        }
    }
}

#[test]
fn the_side_further_along_wins_the_progress_and_notes_are_merged() {
    let base = json!({"tasks": [{"id": "T-1", "status": "todo", "notes": []}]});
    let ours = json!({"tasks": [{"id": "T-1", "status": "in_progress", "updated_at": "2026-01-02T00:00:00Z",
        "notes": [{"at": "2026-01-02T00:00:00Z", "text": "a"}]}]});
    let theirs = json!({"tasks": [{"id": "T-1", "status": "done", "updated_at": "2026-01-01T00:00:00Z",
        "notes": [{"at": "2026-01-01T00:00:00Z", "text": "b"}]}]});
    let (merged, conflicts) = merge_task_files(&base, &ours, &theirs);
    assert!(conflicts.is_empty(), "{conflicts:?}");
    assert_eq!(merged["tasks"][0]["status"], json!("done"));
    assert_eq!(merged["tasks"][0]["notes"][0]["text"], json!("b"));
}

#[test]
fn both_sides_adding_the_same_id_renumbers_theirs_and_its_dependents() {
    let base = json!({"tasks": [{"id": "T-001"}]});
    let ours = json!({"tasks": [{"id": "T-001"}, {"id": "T-002", "name": "ours"}]});
    let theirs = json!({"tasks": [{"id": "T-001"}, {"id": "T-002", "name": "theirs"},
        {"id": "T-003", "dependencies": ["T-002"]}]});
    let (merged, conflicts) = merge_task_files(&base, &ours, &theirs);
    assert!(conflicts.is_empty(), "{conflicts:?}");
    let ids: Vec<&str> = merged["tasks"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["id"].as_str().unwrap())
        .collect();
    assert_eq!(ids, ["T-001", "T-002", "T-004", "T-003"]);
    assert_eq!(merged["tasks"][3]["dependencies"], json!(["T-004"]));
    assert_eq!(
        next_free_id("misc", &["misc".into(), "misc-2".into()]),
        "misc-3"
    );
}

const A: &str = "0193a5b2-7c1d-7e2f-8a3b-4c5d6e7f80a1";
const B: &str = "0193a5b2-7c1d-7e2f-8a3b-4c5d6e7f80b2";
const C: &str = "0193a5b2-7c1d-7e2f-8a3b-4c5d6e7f80c3";

/// A keys-format file (ADR-007) with these tasks, next number 5
fn keyed(tasks: Value) -> Value {
    let mut file = json!({"meta": {"format": 2, "key": "ATM", "next_number": 5}});
    file["tasks"] = tasks;
    file
}

fn numbers(merged: &Value) -> Vec<(String, u64)> {
    merged["tasks"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| {
            (
                t["id"].as_str().unwrap()[34..].to_string(),
                t["number"].as_u64().unwrap(),
            )
        })
        .collect()
}

#[test]
fn keyed_two_branches_adding_a_task_keep_both_and_theirs_takes_the_next_number() {
    let base = keyed(json!([{"id": A, "number": 4, "key_final": true}]));
    let ours = keyed(
        json!([{"id": A, "number": 4, "key_final": true}, {"id": B, "number": 5, "key_final": false}]),
    );
    let theirs = keyed(
        json!([{"id": A, "number": 4, "key_final": true}, {"id": C, "number": 5, "key_final": false}]),
    );
    let (merged, conflicts) = merge_task_files(&base, &ours, &theirs);
    assert!(conflicts.is_empty(), "{conflicts:?}");
    assert_eq!(
        numbers(&merged),
        [("a1".into(), 4), ("b2".into(), 5), ("c3".into(), 6)]
    );
    assert_eq!(
        merged["tasks"][2]["aliases"],
        json!(["ATM-5"]),
        "its old key still finds it"
    );
    assert_eq!(merged["tasks"][1].get("aliases"), None);
    assert_eq!(merged["meta"]["next_number"], json!(7));
}

#[test]
fn keyed_a_final_number_never_moves_and_two_final_ones_are_a_conflict() {
    let base = keyed(json!([]));
    // Ours added B as 5 on a branch; theirs is main, where C is 5 for good
    let ours = keyed(json!([{"id": B, "number": 5, "key_final": false}]));
    let theirs = keyed(json!([{"id": C, "number": 5, "key_final": true}]));
    let (merged, conflicts) = merge_task_files(&base, &ours, &theirs);
    assert!(conflicts.is_empty(), "{conflicts:?}");
    assert_eq!(numbers(&merged), [("b2".into(), 6), ("c3".into(), 5)]);
    assert_eq!(merged["tasks"][0]["aliases"], json!(["ATM-5"]));

    let ours = keyed(json!([{"id": B, "number": 5, "key_final": true}]));
    let (merged, conflicts) = merge_task_files(&base, &ours, &theirs);
    assert_eq!(
        conflicts,
        [format!(
            "ATM-5: two final tasks have this number ({B} and {C}); both kept, give one another number by hand"
        )]
    );
    assert_eq!(numbers(&merged), [("b2".into(), 5), ("c3".into(), 5)]);
}

#[test]
fn keyed_one_uuid_added_on_both_branches_differently_is_a_conflict_not_two_tasks() {
    let base = keyed(json!([]));
    let ours = keyed(json!([{"id": B, "number": 5, "name": "x"}]));
    let theirs = keyed(json!([{"id": B, "number": 5, "name": "y"}]));
    let (merged, conflicts) = merge_task_files(&base, &ours, &theirs);
    assert_eq!(
        conflicts,
        [format!("{B}: added on both branches with different fields")]
    );
    assert_eq!(merged["tasks"].as_array().unwrap().len(), 1);
    assert_eq!(merged["tasks"][0]["name"], json!("x"));
}

#[test]
fn a_branch_from_before_the_migration_merges_into_a_migrated_one_task_by_task() {
    let legacy = |status: &str| {
        json!({"meta": {"id": "3f2a9c1e", "project": "Agentic Task Manager", "next_id": 3},
            "tasks": [{"id": "T-001", "status": "done"}, {"id": "T-002", "status": status, "dependencies": ["T-001"]}]})
    };
    let base = legacy("todo");
    // Ours migrated (and changed nothing else); theirs, still old, finished T-002
    let mut ours = base.clone();
    super::super::migrate::to_keys_format(&mut ours, &mut json!({"tasks": []}), &mut Map::new())
        .unwrap();
    let theirs = legacy("done");
    let (merged, conflicts) = merge_task_files(&base, &ours, &theirs);
    assert!(conflicts.is_empty(), "{conflicts:?}");
    assert_eq!(merged["meta"]["format"], json!(2));
    assert_eq!(merged["meta"]["key"], json!("ATM"));
    let tasks = merged["tasks"].as_array().unwrap();
    assert_eq!(
        tasks.len(),
        2,
        "the same two tasks, matched by their worked-out UUIDs"
    );
    assert_eq!(tasks[1]["number"], json!(2));
    assert_eq!(
        tasks[1]["status"],
        json!("done"),
        "theirs' change came through"
    );
    assert_eq!(tasks[1]["dependencies"], json!([tasks[0]["id"]]));
    // And the other way round
    let (merged, conflicts) = merge_task_files(&base, &theirs, &ours);
    assert!(conflicts.is_empty(), "{conflicts:?}");
    assert_eq!(merged["meta"]["format"], json!(2));
    assert_eq!(merged["tasks"][1]["status"], json!("done"));
}

#[test]
fn a_field_both_sides_changed_differently_is_a_conflict_and_ours_is_kept() {
    let base = json!({"tasks": [{"id": "T-1", "description": "a"}]});
    let ours = json!({"tasks": [{"id": "T-1", "description": "b"}]});
    let theirs = json!({"tasks": [{"id": "T-1", "description": "c"}]});
    let (merged, conflicts) = merge_task_files(&base, &ours, &theirs);
    assert_eq!(conflicts, ["T-1: both branches changed description"]);
    assert_eq!(merged["tasks"][0]["description"], json!("b"));
}

/// Merge three versions of one task T-1 (each a task's fields without its ID): the merged task and the conflicts.
fn merge_one(base: Value, ours: Value, theirs: Value) -> (Value, Vec<String>) {
    let file = |mut task: Value| {
        task["id"] = json!("T-1");
        json!({"tasks": [task]})
    };
    let (merged, conflicts) = merge_task_files(&file(base), &file(ours), &file(theirs));
    assert_eq!(merged["tasks"].as_array().unwrap().len(), 1, "{merged}");
    (merged["tasks"][0].clone(), conflicts)
}

#[test]
fn a_field_one_side_removed_and_the_other_left_alone_is_removed() {
    for (ours, theirs) in [
        (json!({}), json!({"description": "a"})),
        (json!({"description": "a"}), json!({})),
    ] {
        let (task, conflicts) = merge_one(json!({"description": "a"}), ours, theirs);
        assert!(conflicts.is_empty(), "{conflicts:?}");
        assert_eq!(task.get("description"), None);
    }
}

#[test]
fn a_field_one_side_changed_comes_through_from_either_side() {
    let (task, conflicts) = merge_one(
        json!({"name": "a", "description": "a"}),
        json!({"name": "b", "description": "a"}),
        json!({"name": "a", "description": "c"}),
    );
    assert!(conflicts.is_empty(), "{conflicts:?}");
    assert_eq!(
        (&task["name"], &task["description"]),
        (&json!("b"), &json!("c"))
    );
}

#[test]
fn progress_changed_on_one_side_only_comes_from_that_side_whatever_the_rank() {
    // Theirs moved it on; ours only renamed it
    let (task, conflicts) = merge_one(
        json!({"status": "todo", "name": "a"}),
        json!({"status": "todo", "name": "b"}),
        json!({"status": "in_progress", "agent": "t", "name": "a"}),
    );
    assert!(conflicts.is_empty(), "{conflicts:?}");
    assert_eq!(
        (&task["status"], &task["agent"], &task["name"]),
        (&json!("in_progress"), &json!("t"), &json!("b"))
    );
    // Ours moved it back from done (reopened it); theirs only renamed it: ours' progress, though behind
    let (task, _) = merge_one(
        json!({"status": "done", "name": "a"}),
        json!({"status": "todo", "name": "a"}),
        json!({"status": "done", "name": "c"}),
    );
    assert_eq!(
        (&task["status"], &task["name"]),
        (&json!("todo"), &json!("c"))
    );
}

#[test]
fn both_sides_moved_on_the_status_further_along_wins_over_the_newer_update() {
    // (ours, theirs, the winner); ours is always the newer update
    for (ours, theirs, winner) in [
        ("in_progress", "done", "done"),
        ("done", "in_progress", "done"),
        ("skipped", "blocked", "skipped"),
        ("in_review", "todo", "in_review"),
        // A status the vocabulary doesn't know ranks like todo
        ("mystery", "in_progress", "in_progress"),
    ] {
        let (task, conflicts) = merge_one(
            json!({"status": "todo", "updated_at": "2026-01-01T00:00:00Z"}),
            json!({"status": ours, "updated_at": "2026-01-03T00:00:00Z"}),
            json!({"status": theirs, "updated_at": "2026-01-02T00:00:00Z"}),
        );
        assert!(conflicts.is_empty(), "{conflicts:?}");
        assert_eq!(task["status"], json!(winner), "{ours} against {theirs}");
    }
}

#[test]
fn of_two_sides_equally_far_along_the_newer_update_wins_and_on_a_tie_ours() {
    // (ours' update, theirs', the winning agent)
    for (ours, theirs, winner) in [
        ("2026-01-02T00:00:00Z", "2026-01-03T00:00:00Z", "t"),
        ("2026-01-03T00:00:00Z", "2026-01-02T00:00:00Z", "o"),
        ("2026-01-02T00:00:00Z", "2026-01-02T00:00:00Z", "o"),
    ] {
        let (task, conflicts) = merge_one(
            json!({"status": "todo"}),
            json!({"status": "in_progress", "agent": "o", "updated_at": ours}),
            json!({"status": "blocked", "agent": "t", "updated_at": theirs}),
        );
        assert!(conflicts.is_empty(), "{conflicts:?}");
        assert_eq!(task["agent"], json!(winner), "{ours} against {theirs}");
    }
}

#[test]
fn the_progress_fields_move_together_so_none_of_the_losing_sides_come_through() {
    let (task, conflicts) = merge_one(
        json!({"status": "todo"}),
        json!({"status": "in_progress", "agent": "o", "started_at": "2026-01-02T00:00:00Z"}),
        json!({"status": "done", "completed_at": "2026-01-01T00:00:00Z"}),
    );
    assert!(conflicts.is_empty(), "{conflicts:?}");
    assert_eq!(
        task,
        json!({"status": "done", "id": "T-1", "completed_at": "2026-01-01T00:00:00Z"})
    );
}

#[test]
fn notes_from_both_sides_are_kept_once_each_oldest_first() {
    let note =
        |at: &str, text: &str| json!({"at": format!("2026-01-0{at}T00:00:00Z"), "text": text});
    let (task, conflicts) = merge_one(
        json!({"notes": [note("1", "a")]}),
        json!({"notes": [note("3", "c"), note("1", "a")]}),
        json!({"notes": [note("1", "a"), note("2", "b")]}),
    );
    assert!(conflicts.is_empty(), "{conflicts:?}");
    assert_eq!(
        task["notes"],
        json!([note("1", "a"), note("2", "b"), note("3", "c")])
    );
}

#[test]
fn past_blocks_and_reviews_are_kept_one_per_start_oldest_first_ours_where_both_have_it() {
    for key in ["blocks", "reviews"] {
        let (task, conflicts) = merge_one(
            json!({}),
            json!({key: [{"from": "02", "to": "03"}]}),
            json!({key: [{"from": "02", "to": "04"}, {"from": "01"}, "not an interval"]}),
        );
        assert!(conflicts.is_empty(), "{conflicts:?}");
        assert_eq!(
            task[key],
            json!([{"from": "01"}, {"from": "02", "to": "03"}]),
            "{key}"
        );
    }
}

#[test]
fn a_sprint_history_entry_ended_on_either_side_stays_ended() {
    let open = json!({"sprint": "A", "from": "01"});
    let ended = json!({"sprint": "A", "from": "01", "to": "02"});
    let next = json!({"sprint": "B", "from": "02"});
    for (ours, theirs) in [
        (json!([open]), json!([ended, next])),
        (json!([ended, next]), json!([open])),
    ] {
        let (task, conflicts) = merge_one(
            json!({"sprint_history": [open]}),
            json!({"sprint_history": ours}),
            json!({"sprint_history": theirs}),
        );
        assert!(conflicts.is_empty(), "{conflicts:?}");
        assert_eq!(task["sprint_history"], json!([ended, next]));
    }
}

#[test]
fn related_tasks_merge_as_a_set() {
    // (base, ours, theirs, merged: None when nothing is left)
    for (base, ours, theirs, merged) in [
        (
            json!([]),
            json!(["a"]),
            json!(["b"]),
            Some(json!(["a", "b"])),
        ),
        (
            json!(["a", "b"]),
            json!(["a"]),
            json!(["a", "b", "c"]),
            Some(json!(["a", "c"])),
        ),
        (
            json!(["a"]),
            json!(["a", "b"]),
            json!(["b", "a"]),
            Some(json!(["a", "b"])),
        ),
        (json!(["a"]), json!([]), json!(["a"]), None),
        (json!(["a"]), json!(["a"]), json!([]), None),
    ] {
        let (task, conflicts) = merge_one(
            json!({"related": base}),
            json!({"related": ours}),
            json!({"related": theirs}),
        );
        assert!(conflicts.is_empty(), "{conflicts:?}");
        assert_eq!(task.get("related"), merged.as_ref(), "{ours} with {theirs}");
    }
}

#[test]
fn a_phase_both_sides_changed_gives_way_to_a_sprint_on_either_side() {
    for (ours, theirs) in [
        (
            json!({"phase": "p2", "sprint": "S"}),
            json!({"phase": "p3"}),
        ),
        (
            json!({"phase": "p2"}),
            json!({"phase": "p3", "sprint": "S"}),
        ),
    ] {
        let (task, conflicts) = merge_one(json!({"phase": "p1"}), ours, theirs);
        assert!(conflicts.is_empty(), "{conflicts:?}");
        assert_eq!(task.get("phase"), None);
        assert_eq!(task["sprint"], json!("S"));
    }
    // Without a sprint it is a conflict like any other field
    let (task, conflicts) = merge_one(
        json!({"phase": "p1"}),
        json!({"phase": "p2"}),
        json!({"phase": "p3"}),
    );
    assert_eq!(conflicts, ["T-1: both branches changed phase"]);
    assert_eq!(task["phase"], json!("p2"));
}

#[test]
fn a_task_deleted_on_one_side_stays_deleted_unless_the_other_changed_it() {
    let task = |name: &str| json!([{"id": "T-1", "name": name}]);
    // (ours' tasks, theirs', the merged tasks, the conflicts)
    for (ours, theirs, merged, conflicts) in [
        (
            task("b"),
            json!([]),
            task("b"),
            vec!["T-1: changed on this branch, deleted on the other"],
        ),
        (task("a"), json!([]), json!([]), vec![]),
        (
            json!([]),
            task("b"),
            task("b"),
            vec!["T-1: deleted on this branch, changed on the other"],
        ),
        (json!([]), task("a"), json!([]), vec![]),
    ] {
        let (result, found) = merge_task_files(
            &json!({"tasks": task("a")}),
            &json!({"tasks": ours}),
            &json!({"tasks": theirs}),
        );
        assert_eq!(found, conflicts, "{ours} with {theirs}");
        assert_eq!(result["tasks"], merged, "{ours} with {theirs}");
    }
}

#[test]
fn the_same_task_added_on_both_sides_is_kept_once() {
    let added = json!({"tasks": [{"id": "T-2", "name": "n"}]});
    let (merged, conflicts) = merge_task_files(&json!({"tasks": []}), &added, &added);
    assert!(conflicts.is_empty(), "{conflicts:?}");
    assert_eq!(merged["tasks"], added["tasks"]);
}

#[test]
fn a_clashing_id_takes_the_next_number_of_its_pattern_or_a_suffix() {
    let used = |ids: &[&str]| ids.iter().map(ToString::to_string).collect::<Vec<_>>();
    for (id, taken, next) in [
        ("T-009", used(&["T-009", "T-010"]), "T-011"),
        ("T-9", used(&["T-9"]), "T-10"),
        ("T-001", used(&["T-001", "X-500", "T-01a", "T-"]), "T-002"),
        ("misc", used(&["misc"]), "misc-2"),
        ("misc", used(&["misc", "misc-2", "misc-3"]), "misc-4"),
    ] {
        assert_eq!(next_free_id(id, &taken), next, "{id} among {taken:?}");
    }
}

#[test]
fn meta_values_merge_three_ways_and_one_both_changed_keeps_ours_silently() {
    let (merged, conflicts) = merge_task_files(
        &json!({"meta": {"project": "p", "a": 1, "gone": 1, "both": 1}, "tasks": []}),
        &json!({"meta": {"project": "q", "a": 1, "gone": 1, "both": 2}, "tasks": []}),
        &json!({"meta": {"project": "p", "a": 2, "new": 3, "both": 3}, "tasks": []}),
    );
    assert!(conflicts.is_empty(), "{conflicts:?}");
    assert_eq!(
        merged["meta"],
        json!({"project": "q", "a": 2, "both": 2, "new": 3})
    );
}

#[test]
fn a_document_level_key_other_than_tasks_and_meta_is_ours_else_theirs() {
    let (merged, _) = merge_task_files(
        &json!({"tasks": []}),
        &json!({"version": 1, "tasks": []}),
        &json!({"version": 2, "extra": "x", "tasks": []}),
    );
    assert_eq!(merged, json!({"version": 1, "tasks": [], "extra": "x"}));
}

#[test]
fn meta_list_items_merge_item_by_item_with_each_conflict_in_words() {
    let c = |name: &str| json!({"id": "c", "name": name});
    // (base, ours, theirs, the merged items, the conflicts)
    for (base, ours, theirs, items, conflicts) in [
        (
            json!([]),
            json!([c("x")]),
            json!([c("y")]),
            json!([c("x")]),
            vec!["meta.components: 'c' was added on both branches, differently"],
        ),
        (
            json!([]),
            json!([c("x")]),
            json!([c("x")]),
            json!([c("x")]),
            vec![],
        ),
        (
            json!([]),
            json!([]),
            json!([c("x")]),
            json!([c("x")]),
            vec![],
        ),
        (
            json!([c("x")]),
            json!([c("y")]),
            json!([]),
            json!([c("y")]),
            vec!["meta.components: 'c' changed on this branch, deleted on the other"],
        ),
        (
            json!([c("x")]),
            json!([c("x")]),
            json!([]),
            json!([]),
            vec![],
        ),
        (
            json!([c("x")]),
            json!([]),
            json!([c("y")]),
            json!([c("y")]),
            vec!["meta.components: 'c' deleted on this branch, changed on the other"],
        ),
        (
            json!([c("x")]),
            json!([]),
            json!([c("x")]),
            json!([]),
            vec![],
        ),
        (
            json!([c("x")]),
            json!([c("y")]),
            json!([c("z")]),
            json!([c("y")]),
            vec!["meta.components: both branches changed 'c' (name)"],
        ),
    ] {
        let doc = |items: &Value| json!({"meta": {"components": items}, "tasks": []});
        let (merged, found) = merge_task_files(&doc(&base), &doc(&ours), &doc(&theirs));
        assert_eq!(found, conflicts, "{ours} with {theirs}");
        assert_eq!(merged["meta"]["components"], items, "{ours} with {theirs}");
    }
}

#[test]
fn sprint_ids_that_differ_only_in_case_are_one_sprint() {
    let doc = |id: &str, goal: &str| json!({"meta": {"sprints": [{"id": id, "goal": goal}]}, "tasks": []});
    let (merged, conflicts) = merge_task_files(&doc("S", "g"), &doc("S", "h"), &doc("s", "g"));
    assert!(conflicts.is_empty(), "{conflicts:?}");
    assert_eq!(merged["meta"]["sprints"], json!([{"id": "s", "goal": "h"}]));
}

#[test]
fn of_an_item_whose_state_both_sides_moved_the_one_further_along_wins_the_fields_both_changed() {
    let doc = |name: &str, state: &str, notes: &str| {
        let mut meta = json!({});
        meta[name] = json!([{"id": "R", "state": state, "notes": notes}]);
        json!({"meta": meta, "tasks": []})
    };
    // (list, base state, ours, theirs, the winning state and notes)
    for (name, base, ours, theirs, winner) in [
        (
            "releases",
            "planned",
            "released",
            "archived",
            ("archived", "t"),
        ),
        (
            "releases",
            "planned",
            "archived",
            "released",
            ("archived", "o"),
        ),
        ("sprints", "planned", "closed", "active", ("closed", "o")),
        ("sprints", "planned", "active", "closed", ("closed", "t")),
    ] {
        let (merged, conflicts) = merge_task_files(
            &doc(name, base, "b"),
            &doc(name, ours, "o"),
            &doc(name, theirs, "t"),
        );
        assert!(
            conflicts.is_empty(),
            "{name} {ours} {theirs}: {conflicts:?}"
        );
        let item = &merged["meta"][name][0];
        assert_eq!(
            (
                item["state"].as_str().unwrap(),
                item["notes"].as_str().unwrap()
            ),
            winner,
            "{name} {ours} {theirs}"
        );
    }
}

#[test]
fn of_an_item_whose_state_is_equally_far_along_a_field_both_changed_is_a_conflict() {
    let doc = |state: &str, goal: &str| json!({"meta": {"sprints": [{"id": "S", "state": state, "goal": goal}]}, "tasks": []});
    let (merged, conflicts) = merge_task_files(
        &doc("planned", "a"),
        &doc("active", "b"),
        &doc("active", "c"),
    );
    assert_eq!(
        conflicts,
        ["meta.sprints: both branches changed 'S' (goal)"]
    );
    assert_eq!(merged["meta"]["sprints"][0]["goal"], json!("b"));
    // Components have no states: a field both changed is always a conflict
    let doc =
        |state: &str| json!({"meta": {"components": [{"id": "c", "state": state}]}, "tasks": []});
    let (_, conflicts) = merge_task_files(&doc("a"), &doc("b"), &doc("c"));
    assert_eq!(
        conflicts,
        ["meta.components: both branches changed 'c' (state)"]
    );
}

#[test]
fn a_project_migrated_to_sprints_on_either_side_has_no_phases_left() {
    let phases = json!([{"id": "p1"}]);
    let (merged, conflicts) = merge_task_files(
        &json!({"meta": {"phases": phases}, "tasks": []}),
        &json!({"meta": {"phases": phases}, "tasks": []}),
        &json!({"meta": {"phases": phases, "sprints": [{"id": "S"}]}, "tasks": []}),
    );
    assert!(conflicts.is_empty(), "{conflicts:?}");
    assert_eq!(merged["meta"], json!({"sprints": [{"id": "S"}]}));
}

#[test]
fn keyed_a_renumbered_task_goes_above_either_sides_next_number_and_keeps_its_aliases() {
    let base = keyed(json!([]));
    let ours = keyed(json!([{"id": B, "number": 5, "key_final": false}]));
    let mut theirs =
        keyed(json!([{"id": C, "number": 5, "key_final": false, "aliases": ["OLD-1"]}]));
    theirs["meta"]["next_number"] = json!(9);
    let (merged, conflicts) = merge_task_files(&base, &ours, &theirs);
    assert!(conflicts.is_empty(), "{conflicts:?}");
    assert_eq!(numbers(&merged), [("b2".into(), 5), ("c3".into(), 9)]);
    assert_eq!(merged["tasks"][1]["aliases"], json!(["OLD-1", "ATM-5"]));
    assert_eq!(merged["meta"]["next_number"], json!(10));
}

#[test]
fn keyed_the_next_number_is_never_below_the_highest_number_plus_one() {
    let file = |next: u64| {
        let mut file = keyed(json!([{"id": A, "number": 7, "key_final": true}]));
        file["meta"]["next_number"] = json!(next);
        file
    };
    // (the stored next number on both sides, the merged one)
    for (stored, next) in [(1, 8), (8, 8), (12, 12)] {
        let (merged, _) = merge_task_files(&file(stored), &file(stored), &file(stored));
        assert_eq!(
            merged["meta"]["next_number"],
            json!(next),
            "stored {stored}"
        );
    }
}

fn write(dir: &Path, name: &str, text: &str) -> std::path::PathBuf {
    let path = dir.join(name);
    std::fs::write(&path, text).unwrap();
    path
}

#[test]
fn the_merge_driver_writes_the_merged_file_over_ours_and_returns_the_conflicts() {
    let dir = crate::test_dir::TestDir::new();
    let base = write(
        dir.path(),
        "base",
        r#"{"tasks": [{"id": "T-1", "name": "a"}]}"#,
    );
    let ours = write(
        dir.path(),
        "ours",
        r#"{"tasks": [{"id": "T-1", "name": "b"}]}"#,
    );
    let theirs = write(
        dir.path(),
        "theirs",
        r#"{"tasks": [{"id": "T-1", "name": "c"}, {"id": "T-2"}]}"#,
    );
    let conflicts = merge_files(&base, &ours, &theirs).unwrap();
    assert_eq!(conflicts, ["T-1: both branches changed name"]);
    let text = std::fs::read_to_string(&ours).unwrap();
    assert!(text.ends_with('\n'));
    let written: Value = serde_json::from_str(&text).unwrap();
    assert_eq!(
        written,
        json!({"tasks": [{"id": "T-1", "name": "b"}, {"id": "T-2"}]})
    );
}

#[test]
fn the_merge_driver_reads_an_empty_base_as_a_file_added_on_both_branches() {
    let dir = crate::test_dir::TestDir::new();
    let base = write(dir.path(), "base", "");
    let ours = write(dir.path(), "ours", r#"{"tasks": [{"id": "T-1"}]}"#);
    let theirs = write(dir.path(), "theirs", r#"{"tasks": [{"id": "T-2"}]}"#);
    assert_eq!(
        merge_files(&base, &ours, &theirs).unwrap(),
        Vec::<String>::new()
    );
    let written: Value = serde_json::from_str(&std::fs::read_to_string(&ours).unwrap()).unwrap();
    assert_eq!(written["tasks"], json!([{"id": "T-1"}, {"id": "T-2"}]));
}

#[test]
fn the_merge_driver_refuses_a_side_that_is_not_task_data_and_leaves_ours_alone() {
    let good = r#"{"tasks": []}"#;
    // (which side is bad, its text: None for a missing file)
    for (side, bad) in [
        (0, Some("{not json")),
        (1, Some("[1, 2]")),
        (2, Some(r#"{"tasks": {}}"#)),
        (1, None),
        (2, None),
    ] {
        let dir = crate::test_dir::TestDir::new();
        let paths: Vec<_> = ["base", "ours", "theirs"]
            .iter()
            .enumerate()
            .map(|(i, name)| match (i == side, bad) {
                (true, None) => dir.path().join(name),
                (true, Some(text)) => write(dir.path(), name, text),
                (false, _) => write(dir.path(), name, good),
            })
            .collect();
        let error = merge_files(&paths[0], &paths[1], &paths[2]).unwrap_err();
        assert!(
            error.to_string().contains("resolve the conflict by hand"),
            "{side} {bad:?}: {error}"
        );
        let ours_now = std::fs::read_to_string(&paths[1]).ok();
        let ours_before = if side == 1 {
            bad.map(str::to_string)
        } else {
            Some(good.to_string())
        };
        assert_eq!(ours_now, ours_before, "{side} {bad:?}");
    }
}

#[test]
fn a_sprint_closed_on_one_branch_stays_closed() {
    let meta = |state: &str, goal: &str| json!({"meta": {"sprints": [{"id": "S", "state": state, "goal": goal}]}, "tasks": []});
    let (merged, conflicts) = merge_task_files(
        &meta("active", "g"),
        &meta("closed", "g"),
        &meta("planned", "h"),
    );
    assert!(conflicts.is_empty(), "{conflicts:?}");
    assert_eq!(merged["meta"]["sprints"][0]["state"], json!("closed"));
}
