//! `atm version` and `atm docs` (and, with its slice, `atm help`): the commands about atm itself. The Python atm's
//! `cli/commands/meta.py`.

use atm_common::{Error, Result};
use serde_json::{Value, json};

use super::{Context, print_json};

include!(concat!(env!("OUT_DIR"), "/agent_docs.rs"));

/// `atm version`: atm's version, as text or `{"version": …}`.
pub fn version(ctx: Context) {
    if ctx.json {
        print_json(&json!({"version": atm_common::VERSION}));
    } else {
        println!("{}", atm_common::VERSION);
    }
}

/// A topic's text, ending in one newline.
fn text_of(raw: &str) -> String {
    format!("{}\n", raw.trim_end_matches('\n'))
}

/// `atm docs [topic]`: the reference for agents that matches this version of atm; without a topic, the topics.
///
/// # Errors
/// `Invalid` for a topic atm doesn't have.
pub fn docs(topic: Option<&str>, ctx: Context) -> Result<()> {
    let Some(topic) = topic else {
        if ctx.json {
            let topics: Vec<Value> = AGENT_DOCS
                .iter()
                .map(|(name, summary, _)| json!({"topic": name, "summary": summary}))
                .collect();
            print_json(&Value::Array(topics));
        } else {
            let width = AGENT_DOCS
                .iter()
                .map(|(name, _, _)| name.len())
                .max()
                .unwrap_or(0);
            let lines: Vec<String> = AGENT_DOCS
                .iter()
                .map(|(name, summary, _)| format!("{name:<width$}  {summary}"))
                .collect();
            println!("{}", lines.join("\n"));
        }
        return Ok(());
    };
    let Some((name, summary, raw)) = AGENT_DOCS.iter().find(|(name, _, _)| *name == topic) else {
        let names: Vec<&str> = AGENT_DOCS.iter().map(|(name, _, _)| *name).collect();
        return Err(Error::Invalid(format!(
            "no topic '{topic}' (atm docs lists them: {})",
            names.join(", ")
        )));
    };
    let text = text_of(raw);
    if ctx.json {
        print_json(&json!({"topic": name, "summary": summary, "text": text}));
    } else {
        println!("{}", text.trim_end_matches('\n'));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_topics_are_built_in_sorted_with_compmd() {
        let names: Vec<&str> = AGENT_DOCS.iter().map(|(n, _, _)| *n).collect();
        assert!(names.windows(2).all(|w| w[0] < w[1]), "{names:?}");
        assert!(names.contains(&"compmd") && names.contains(&"rules"));
        assert!(
            AGENT_DOCS
                .iter()
                .all(|(_, summary, text)| !summary.is_empty() && !text.is_empty())
        );
    }

    #[test]
    fn a_topic_ends_in_one_newline() {
        assert_eq!(text_of("a\n\n\n"), "a\n");
        assert_eq!(text_of("a"), "a\n");
    }

    #[test]
    fn an_unknown_topic_lists_the_topics() {
        let error = docs(
            Some("nope"),
            Context {
                json: false,
                dry_run: false,
            },
        )
        .unwrap_err();
        assert_eq!(error.code(), 2);
        assert!(
            error
                .to_string()
                .starts_with("no topic 'nope' (atm docs lists them: ")
        );
    }
}
