//! Who worked on the project (docs/project-stats-proposal.md §3, T-252): the people, with the tasks they created,
//! started and completed, most active first; and the agents in one summary. Tasks from before atm recorded who started
//! them count as agent work, as on the Agents tab.

use std::collections::{BTreeMap, HashSet};

use atm_common::Kind;
use atm_common::edits::is_agent;
use serde_json::{Value, json};

use super::Item;
use super::time::time_use;

#[derive(Default)]
struct Person {
    created: usize,
    started: usize,
    completed: usize,
}

/// `{people: [{name, created, started, completed}], agents: {sessions, started, completed, agent_ms}}`.
pub(super) fn actors(items: &[&Item], now: i64) -> Value {
    let mut people: BTreeMap<String, Person> = BTreeMap::new();
    let mut sessions: HashSet<String> = HashSet::new();
    let (mut by_agents, mut completed) = (0usize, 0usize);
    let mut agent_work: Vec<&Item> = Vec::new();
    let text = |item: &Item, field: &str| {
        item.task
            .get(field)
            .and_then(Value::as_str)
            .map(String::from)
    };
    for item in items {
        let starter = item.started_by().map(String::from);
        let created_by = text(item, "created_by");
        let completed_by = text(item, "completed_by");
        if let Some(by) = created_by.as_ref().filter(|b| !is_agent(Some(b))) {
            people.entry(by.clone()).or_default().created += 1;
        }
        let agent_started = match &starter {
            Some(by) if !is_agent(Some(by)) => {
                people.entry(by.clone()).or_default().started += 1;
                false
            }
            Some(by) => {
                sessions.insert(by.clone());
                true
            }
            None => item.time("started_at").is_some(),
        };
        if agent_started {
            by_agents += 1;
            agent_work.push(item);
        }
        // Before v4 nobody recorded who completed a task: a finished task an agent started counts as the agent's
        match &completed_by {
            Some(by) if !is_agent(Some(by)) => people.entry(by.clone()).or_default().completed += 1,
            Some(by) => {
                completed += 1;
                sessions.insert(by.clone());
            }
            None if agent_started && item.kind == Some(Kind::Done) => completed += 1,
            None => {}
        }
        if let Some(by) = created_by.filter(|b| is_agent(Some(b))) {
            sessions.insert(by);
        }
    }
    let mut listed: Vec<(String, Person)> = people.into_iter().collect();
    listed.sort_by(|a, b| {
        let total = |p: &Person| p.created + p.started + p.completed;
        total(&b.1).cmp(&total(&a.1)).then_with(|| a.0.cmp(&b.0))
    });
    json!({
        "people": listed.into_iter().map(|(name, p)| json!({
            "name": name, "created": p.created, "started": p.started, "completed": p.completed,
        })).collect::<Vec<_>>(),
        "agents": {
            "sessions": sessions.len(),
            "started": by_agents,
            "completed": completed,
            "agent_ms": time_use(&agent_work, None, now).map_or(0, |u| u.agent_ms),
        },
    })
}
