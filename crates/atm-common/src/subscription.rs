//! Being told when a store's data changes (ADR-008 §5, T-555): the callback and the handle every store's `subscribe`
//! hands out, the task store's (atm-db) and the review store's (atm-review) alike, so atm-lib can merge them. How a
//! store hears of a change is its own business; these only carry the news.

use std::sync::Arc;

/// What a subscription calls when the data changed: only that it did, never what changed (the caller reads it again).
/// It runs on the store's own thread (a watcher's, a poller's, or the writer's), so it must not wait on a lock its
/// caller may hold while subscribing or reading; shared, so one callback can serve several subscriptions merged.
pub type OnChange = Arc<dyn Fn() + Send + Sync>;

/// A live subscription: it tells its [`OnChange`] until it is dropped.
#[must_use = "dropping a subscription ends it"]
pub struct Subscription {
    /// What keeps it going (a watcher, a poller's stop signal, a listener's registration): held only, dropped to end it
    held: Vec<Box<dyn Send>>,
}

impl std::fmt::Debug for Subscription {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Subscription")
            .field("parts", &self.held.len())
            .finish()
    }
}

impl Subscription {
    /// A subscription kept going by `held`, which ends it when dropped.
    pub fn new(held: impl Send + 'static) -> Self {
        Self {
            held: vec![Box::new(held)],
        }
    }

    /// Several subscriptions as one: dropping it ends them all.
    pub fn merged(parts: impl IntoIterator<Item = Subscription>) -> Self {
        Self {
            held: parts.into_iter().flat_map(|p| p.held).collect(),
        }
    }
}
