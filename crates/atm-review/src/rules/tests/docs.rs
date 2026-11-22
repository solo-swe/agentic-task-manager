//! ATM-715 (docs/task-review-state-machine.md §7 step 5): what `atm docs reviews` says of a review's status is the
//! table's: each row of its table is checked against the review table's rows, both ways, so a change to the table
//! fails here until the docs follow.

use super::{TABLE_STATUSES, review_rows};
use crate::model::Status;

/// The agent docs' reviews topic, as atm embeds it.
const REVIEWS: &str = include_str!("../../../../../docs/agent/reviews.md");

/// The docs' events, by the name the review table's rows give them.
const EVENTS: [(&str, &str); 5] = [
    ("round", "round"),
    ("approve", "approve"),
    ("request changes", "request"),
    ("comment", "comment"),
    ("close", "close"),
];

fn status_named(name: &str) -> Status {
    TABLE_STATUSES
        .into_iter()
        .find(|s| s.name() == name)
        .unwrap_or_else(|| panic!("the docs name a status atm hasn't: {name}"))
}

/// The docs' table: (from, the table's event, to; `None`: it stays), a row a status it moves from.
fn documented() -> Vec<(Status, &'static str, Option<Status>)> {
    let section = REVIEWS
        .split("## How a review's status moves")
        .nth(1)
        .expect("atm docs reviews has its table of how a review's status moves");
    let mut rows = Vec::new();
    for line in section.lines().filter(|l| l.starts_with('|')) {
        let cells: Vec<&str> = line.trim_matches('|').split('|').map(str::trim).collect();
        let [event, from, to] = cells[..] else {
            panic!("a row of three cells: {line}");
        };
        if event == "Event" || event.starts_with("---") {
            continue;
        }
        let (_, name) = EVENTS
            .iter()
            .find(|(doc, _)| *doc == event)
            .unwrap_or_else(|| panic!("the docs name an event the table hasn't: {event}"));
        for from in from.split(',').map(str::trim) {
            let from = status_named(from);
            let to = match to {
                "stays" => None,
                named if status_named(named) == from => None,
                named => Some(status_named(named)),
            };
            rows.push((from, *name, to));
        }
    }
    rows
}

#[test]
fn what_atm_docs_reviews_says_of_a_reviews_status_is_the_tables() {
    let table = review_rows();
    let documented = documented();
    // Each row the docs give is the table's
    for (from, event, to) in &documented {
        let row = table
            .iter()
            .find(|(status, name, _)| status == from && name == event)
            .unwrap_or_else(|| panic!("no table row for {} × {event}", from.name()));
        assert_eq!(
            row.2,
            Ok(*to),
            "the docs say {} × {event} leads to {to:?}",
            from.name()
        );
    }
    // Each move the table allows for these events is in the docs, and a refused one isn't
    for (status, event, next) in &table {
        if !EVENTS.iter().any(|(_, name)| name == event) {
            continue;
        }
        let said = documented
            .iter()
            .any(|(from, name, _)| from == status && name == event);
        assert_eq!(
            said,
            next.is_ok(),
            "{} × {event}: the table {} it, the docs {}",
            status.name(),
            if next.is_ok() { "allows" } else { "refuses" },
            if said { "list it" } else { "don't" }
        );
    }
}
