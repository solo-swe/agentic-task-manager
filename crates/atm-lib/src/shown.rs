//! What a page shows of a project, and whether it changed (ATM-767, docs/memory-growth-analysis.md §3b): a change of
//! the project's folders that leaves it the same is no change for the page. Every atm command an agent runs renews its
//! lease, rewriting the claims store; the page shows a claim's holder, not its lease's end, so a renewal alone reloads
//! nothing. The app's subscriptions are told of a project's changes through [`on_shown_change`].

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use atm_common::subscription::OnChange;
use serde_json::{Map, Value};

use crate::project::Project;

/// A claim's fields a renewal moves: the page shows neither
const LEASE_FIELDS: [&str; 2] = ["lease_expires_at", "renewed_at"];

/// The claims store as the page sees it: each claim without its lease's end and its last renewal.
#[must_use]
pub fn shown_claims(claims: &Map<String, Value>) -> Value {
    let mut shown = claims.clone();
    if let Some(Value::Object(held)) = shown.get_mut("claims") {
        for claim in held.values_mut().filter_map(Value::as_object_mut) {
            for field in LEASE_FIELDS {
                claim.shift_remove(field);
            }
        }
    }
    Value::Object(shown)
}

/// What the page shows of the project at `root`, cheap to read: the store's stamp (its files' sizes and times), the
/// claims store's by what it holds but the leases' ends. Null when the project can't be opened.
#[must_use]
pub fn shown_stamp(root: &Path) -> Value {
    let Ok(project) = Project::open(root) else {
        return Value::Null;
    };
    let mut stamp = project.store().stamp();
    if let Some(parts) = stamp.as_object_mut() {
        parts.insert(
            "claims".into(),
            shown_claims(&project.store().read_claims()),
        );
    }
    stamp
}

/// `on_change`, called only when what the page shows of the project at `root` changed since the last call (or the
/// subscription's start): a lease renewed, or a file written the same, calls nothing.
#[must_use]
pub fn on_shown_change(root: PathBuf, on_change: OnChange) -> OnChange {
    let last = Mutex::new(shown_stamp(&root));
    Arc::new(move || {
        let now = shown_stamp(&root);
        let changed = {
            let mut last = last
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            let changed = *last != now;
            *last = now;
            changed
        };
        if changed {
            on_change();
        }
    })
}

#[cfg(test)]
mod tests;
