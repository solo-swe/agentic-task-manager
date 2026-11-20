//! The inbox's line on every command (ATM-708).

use super::*;

/// ATM-708: the line every command prints while something is unread
#[test]
fn the_unread_line_names_the_first_three_and_counts_the_rest_and_is_none_without_any() {
    let comment = |n: u32| json!({"review": format!("CR-{n}"), "kind": "comment", "author": "Ana"});
    let poke = json!({"review": "CR-5", "kind": "poke", "author": "Ana"});
    let decided = json!({"review": "CR-6", "decision": "changes_requested"});
    assert_eq!(unread_summary(&[]), None);
    assert_eq!(
        unread_summary(&[comment(1), decided.clone()]).unwrap(),
        "2 unread about your work: CR-1 comment by Ana, CR-6 changes requested. `atm review inbox` shows them \
         (and marks them read)"
    );
    let many = unread_summary(&[comment(1), poke, decided, comment(7), comment(8)]).unwrap();
    assert!(
        many.starts_with("5 unread about your work: CR-1 comment by Ana, CR-5 poke by Ana, CR-6 changes requested, 2 more."),
        "{many}"
    );
}
