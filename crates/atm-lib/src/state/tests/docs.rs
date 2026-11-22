//! ATM-715 (docs/task-review-state-machine.md §7 step 5): what `atm docs states` says of a task's moves is the table's
//! (`Command::sources` and `destination`): every command has its row, each row the kinds the table gives it, so a change
//! to the table fails here until the docs follow.

use super::super::Command;
use atm_common::vocabulary::Kind;

/// The agent docs' states topic, as atm embeds it.
const STATES: &str = include_str!("../../../../../docs/agent/states.md");

fn kind_named(name: &str) -> Kind {
    Kind::parse(name).unwrap_or_else(|| panic!("the docs name a kind atm hasn't: {name}"))
}

/// The docs' table: (move, from, to), a row a move.
fn documented() -> Vec<(String, Vec<Kind>, Kind)> {
    let section = STATES
        .split("## The table")
        .nth(1)
        .and_then(|rest| rest.split("\n## ").next())
        .expect("atm docs states has its table of moves");
    section
        .lines()
        .filter(|l| l.starts_with('|'))
        .filter_map(|line| {
            let cells: Vec<&str> = line.trim_matches('|').split('|').map(str::trim).collect();
            let [name, from, to] = cells[..] else {
                panic!("a row of three cells: {line}");
            };
            (name != "Move" && !name.starts_with("---")).then(|| {
                let from = from.split(',').map(|k| kind_named(k.trim())).collect();
                (name.to_string(), from, kind_named(to))
            })
        })
        .collect()
}

#[test]
fn what_atm_docs_states_says_of_a_tasks_moves_is_the_tables() {
    let documented = documented();
    for command in Command::ALL {
        let row = documented
            .iter()
            .find(|(name, _, _)| name == command.name())
            .unwrap_or_else(|| panic!("atm docs states has no row for {}", command.name()));
        assert_eq!(
            row.1,
            command.sources(),
            "{}: the kinds it moves from",
            command.name()
        );
        assert_eq!(
            row.2,
            command.destination(),
            "{}: the kind it leaves",
            command.name()
        );
    }
    // No row for a move atm hasn't
    for (name, _, _) in &documented {
        assert!(
            Command::ALL.iter().any(|c| c.name() == name),
            "atm docs states names a move atm hasn't: {name}"
        );
    }
}
