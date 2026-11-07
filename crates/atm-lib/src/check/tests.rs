use serde_json::json;

use super::*;

#[test]
fn a_good_file_has_nothing_to_say() {
    let document = json!({"tasks": [
        {"id": "T-1", "name": "a", "type": "feature", "status": "done"},
        {"id": "T-2", "name": "b", "type": "feature", "status": "todo", "dependencies": ["T-1", "T-0"]}
    ]});
    let archived = [json!({"id": "T-0"}).as_object().unwrap().clone()];
    assert_eq!(
        check_data(&document, None, false, &archived),
        (Vec::new(), Vec::new())
    );
}

#[test]
fn errors_and_warnings_are_named() {
    let document = json!({"tasks": [
        {"id": "T-1", "name": "a", "type": "odd", "status": "nope", "priority": "P9", "labels": ["Bad"]},
        {"id": "T-1", "name": "b", "type": "feature", "status": "todo", "dependencies": ["T-2"]},
        {"id": "T-2", "name": "c", "type": "feature", "status": "todo", "dependencies": ["T-1"], "resolution": "done"},
        {"name": "no id"}
    ]});
    let (errors, warnings) = check_data(&document, None, false, &[]);
    assert_eq!(
        errors,
        [
            "T-1: the ID is used by 2 tasks",
            "T-1: the status 'nope' is not one of the project's statuses",
            "T-1: the label 'Bad' isn't lowercase letters, digits, - and _",
            "T-2: has a resolution (done) but isn't done or skipped",
            "task #4: needs an \"id\" (a non-empty string)",
            "dependency cycle: T-1 -> T-2 -> T-1",
        ]
    );
    assert_eq!(
        warnings,
        [
            "T-1: the type 'odd' is not one of the project's types",
            "T-1: the priority 'P9' is not one of the project's priorities",
        ]
    );
}

#[test]
fn an_unusable_vocabulary_is_an_error_and_the_defaults_check_the_rest() {
    let document = json!({"meta": {"types": []}, "tasks": [{"id": "T-1", "name": "a", "type": "feature", "status": "todo"}]});
    let (errors, _) = check_data(&document, None, false, &[]);
    assert_eq!(errors, ["meta.types must be a non-empty list"]);
}
