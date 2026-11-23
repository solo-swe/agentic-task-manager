//! A project's stats, progress and forecast (ATM-722, docs/multi-project-api.md §2.5a), for a slice of its tasks and a
//! period: atm-lib's rules, which the dashboard draws.

use std::collections::BTreeSet;

use atm_common::Error;
use atm_lib::stats::StatsQuery;
use axum::response::Response;

use super::query::parse_query;
use super::source::ProjectSource;
use super::{error_response, ok_json, stamped};

/// The query a route's parameters ask: `tasks` the slice (IDs or keys, comma-separated or repeated; none, the whole
/// project), `from` and `to` the period (ms since 1970).
///
/// # Errors
/// `Invalid` for a period's end that isn't a whole number, or one before its start.
pub(super) fn stats_query(query: &str) -> atm_common::Result<StatsQuery> {
    let given = parse_query(query);
    let mut tasks: Option<BTreeSet<String>> = None;
    let (mut from, mut to) = (None, None);
    for (name, value) in given {
        match name.as_str() {
            "tasks" => tasks.get_or_insert_default().extend(
                value
                    .split(',')
                    .map(str::trim)
                    .filter(|v| !v.is_empty())
                    .map(String::from),
            ),
            "from" | "to" => {
                let at: i64 = value.parse().map_err(|_| {
                    Error::Invalid(format!("{name}: a time in ms since 1970, not {value}"))
                })?;
                *(if name == "from" { &mut from } else { &mut to }) = Some(at);
            }
            _ => {}
        }
    }
    if let (Some(from), Some(to)) = (from, to)
        && to < from
    {
        return Err(Error::Invalid(format!("to ({to}) is before from ({from})")));
    }
    Ok(StatsQuery { tasks, from, to })
}

/// `GET …/projects/<p>/stats`: atm-lib's stats for the query's slice and period. Not cached: the figures run to now
/// (work going on, time idle since the last).
pub(super) fn get_stats(project: &dyn ProjectSource, query: &str) -> Response {
    match stats_query(query).and_then(|q| project.stats(&q)) {
        Ok(stats) => ok_json(&stamped(stats), None),
        Err(e) => error_response(&e),
    }
}

#[cfg(test)]
mod tests {
    use axum::http::StatusCode;
    use serde_json::json;

    use super::super::test_support::{Call, FakeProject, answer, bound};
    use super::*;

    #[test]
    fn the_slice_and_the_period_come_from_the_query() {
        let q = stats_query("tasks=T-1,T-2&tasks=ATM-3&from=10&to=20&x=y").unwrap();
        assert_eq!(
            q.tasks.unwrap().into_iter().collect::<Vec<_>>(),
            ["ATM-3", "T-1", "T-2"]
        );
        assert_eq!((q.from, q.to), (Some(10), Some(20)));
        // None given: the whole project, all time; a blank value is no value, as on every route (the dashboard asks
        // for no slice of none: it shows that no task matches)
        let all = stats_query("").unwrap();
        assert!(all.tasks.is_none() && all.from.is_none() && all.to.is_none());
        assert_eq!(stats_query("tasks=").unwrap().tasks, None);
    }

    #[test]
    fn a_period_that_isnt_one_is_refused() {
        for bad in ["from=soon", "to=1.5", "from=20&to=10"] {
            assert!(matches!(stats_query(bad), Err(Error::Invalid(_))), "{bad}");
        }
    }

    #[test]
    fn the_stats_are_atm_libs_for_the_query_and_its_refusals_by_their_kind() {
        let stats = json!({"progress": {"overall": {"done": 1}}, "forecast": {"pace": null, "sprints": []}});
        let (project, calls) = bound(FakeProject {
            stats: Ok(stats.clone()),
            ..FakeProject::default()
        });
        let ok = answer(get_stats(project.as_ref(), "tasks=T-1&from=5"));
        assert_eq!(ok.status, StatusCode::OK);
        assert_eq!(ok.unstamped(), stats);
        assert_eq!(ok.tag(), "", "never cached: it runs to now");
        assert_eq!(
            calls.taken(),
            [Call::Stats(stats_query("tasks=T-1&from=5").unwrap())]
        );
        // A bad query asks atm-lib nothing
        let bad = answer(get_stats(project.as_ref(), "from=x"));
        assert_eq!(bad.status, StatusCode::BAD_REQUEST);
        assert_eq!(calls.taken(), []);
        for (error, status) in [
            (
                Error::TooLarge("larger than 100 MB".into()),
                StatusCode::PAYLOAD_TOO_LARGE,
            ),
            (
                Error::failed("unreadable"),
                StatusCode::INTERNAL_SERVER_ERROR,
            ),
        ] {
            let (project, _) = bound(FakeProject {
                stats: Err(error.clone()),
                ..FakeProject::default()
            });
            let refused = answer(get_stats(project.as_ref(), ""));
            assert_eq!(refused.status, status, "{error}");
            assert_eq!(refused.json(), json!({"error": error.to_string()}));
        }
    }
}
