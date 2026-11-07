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

#[test]
fn a_field_both_sides_changed_differently_is_a_conflict_and_ours_is_kept() {
    let base = json!({"tasks": [{"id": "T-1", "description": "a"}]});
    let ours = json!({"tasks": [{"id": "T-1", "description": "b"}]});
    let theirs = json!({"tasks": [{"id": "T-1", "description": "c"}]});
    let (merged, conflicts) = merge_task_files(&base, &ours, &theirs);
    assert_eq!(conflicts, ["T-1: both branches changed description"]);
    assert_eq!(merged["tasks"][0]["description"], json!("b"));
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
