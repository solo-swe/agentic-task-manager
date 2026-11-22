//! One view of a review: a round's files, or what changed since an earlier round, each chain placed in it.

use super::{
    Anchor, Chain, Draft, Error, FilePatch, Place, Project, Result, Review, Round, RoundDiffs,
    RoundKind, Value, file_patches, json, reviews, rounds, with_links,
};

pub(super) fn round_of(review: &Review, number: u32) -> Result<&Round> {
    review
        .rounds
        .iter()
        .find(|r| r.number == number)
        .ok_or_else(|| {
            Error::NotFound(format!("{} has no round {number}", review.header.reference))
        })
}

/// The files of one view (ATM-725, [`rounds::between`]): the whole change of round `shown`, from the review's base,
/// or what changed since round `against`, the increments between them added up; each file with its patch and the
/// objects (a commit, or a tree) its two sides are read from.
pub(super) fn view<S: RoundDiffs + ?Sized>(
    vcs: &atm_vcs::GitVcs,
    store: &S,
    review: &Review,
    shown: u32,
    against: Option<u32>,
) -> Result<Value> {
    let round = round_of(review, shown)?;
    if against == Some(shown) {
        return Err(Error::Invalid(format!(
            "round {shown} against itself: name an earlier round"
        )));
    }
    let since = against.map_or(rounds::Since::Base, rounds::Since::Round);
    let made = rounds::between(vcs, store, review, since, shown)?;
    // A files round's whole change starts from nothing: its files' left sides aren't read
    let first_is_files = review
        .rounds
        .iter()
        .min_by_key(|r| r.number)
        .is_some_and(|r| r.kind == RoundKind::Files);
    let before = (!(against.is_none() && first_is_files)).then(|| made.before.to_string());
    let after = made.after.to_string();
    let patch = reviews::Patch(made.diff.patch);
    let listed = rounds::file_changes(made.diff.files);
    let repo = rounds::repository(vcs, round).unwrap_or_else(|_| vcs.clone());
    let patches = with_links(&repo, file_patches(&patch.0), before.as_ref(), &after);
    // A moved submodule's entry: its two commits and its repository's web address, when it has one, for links
    let submodule = |file: &FilePatch| {
        file.submodule.as_ref().map(|(from, to)| {
            let repository = vcs
                .submodule_at(&file.path)
                .and_then(|sub| sub.remote_url())
                .and_then(|url| atm_common::repository::repository_url(&url));
            json!({ "from": from, "to": to, "repository": repository })
        })
    };
    let entry = |file: &FilePatch, before: Option<&String>, after: &String| {
        json!({
            "path": file.path,
            "old_path": file.old_path,
            "status": file.status,
            "binary": file.binary,
            "submodule": submodule(file),
            "patch": file.patch,
            "before": if file.status == "added" { None } else { before.cloned() },
            "after": if file.status == "deleted" { None } else { Some(after.clone()) },
        })
    };
    // A submodule moved from one commit to another: its entry, then the files its own repository changed between
    // them, named as the project has them, their sides the submodule's commits (ATM-599, Hakan on CR-4); none when it
    // isn't checked out here, or a commit is gone from it
    let side = |file: &FilePatch| {
        let mut out = vec![entry(file, before.as_ref(), &after)];
        if let Some((Some(from), Some(to))) = &file.submodule
            && from != to
            && let Some(sub) = vcs.submodule_at(&file.path)
            && let (Ok(a), Ok(b)) = (atm_vcs::Rev::object_id(from), atm_vcs::Rev::object_id(to))
            && let Ok(diff) = sub.diff_range(&a, &b, &[])
        {
            out.extend(
                file_patches(&diff.patch)
                    .iter()
                    .map(|inner| entry(inner, Some(from), to)),
            );
        }
        out
    };
    // Every file the view changes (a binary one has no patch), in the order git lists them
    let files: Vec<Value> = listed
        .iter()
        .flat_map(|f| {
            let found = patches.iter().find(|p| p.path == f.path).cloned();
            side(&found.unwrap_or_else(|| FilePatch {
                path: f.path.clone(),
                old_path: match &f.status {
                    reviews::FileStatus::Renamed { from } => from.clone(),
                    _ => f.path.clone(),
                },
                status: match &f.status {
                    reviews::FileStatus::Added => "added",
                    reviews::FileStatus::Deleted => "deleted",
                    reviews::FileStatus::Renamed { .. } => "renamed",
                    reviews::FileStatus::Modified => "modified",
                },
                binary: f.binary,
                submodule: None,
                patch: String::new(),
            }))
        })
        .collect();
    Ok(json!({ "round": shown, "against": against, "files": files }))
}

/// A chain, with where it shows in round `shown`: `placed` its lines there (none for the review's or a whole file's),
/// `drifted` the round and lines it was made on when its text is gone from there.
pub(super) fn placed<S: RoundDiffs + ?Sized>(
    vcs: &atm_vcs::GitVcs,
    store: &S,
    review: &Review,
    chain: &Chain,
    shown: u32,
    against: Option<u32>,
) -> Value {
    let out = serde_json::to_value(chain).unwrap_or(Value::Null);
    with_placement(vcs, store, review, &chain.anchor, (shown, against), out)
}

/// `out` with where `anchor` shows in the view of round `shown` (since `against`): `placed`, `drifted`, and
/// `in_view`, whether the view holds the increment of the round it was made on (ATM-725, [`rounds::in_view`]): a
/// comment on another round is listed as such, not placed.
pub(super) fn with_placement<S: RoundDiffs + ?Sized>(
    vcs: &atm_vcs::GitVcs,
    store: &S,
    review: &Review,
    anchor: &Anchor,
    (shown, against): (u32, Option<u32>),
    mut out: Value,
) -> Value {
    let since = against.map_or(rounds::Since::Base, rounds::Since::Round);
    let in_view = rounds::in_view(since, shown, anchor.round);
    let made = || json!({ "round": anchor.round, "lines": anchor.lines });
    let (at, drifted) = match rounds::place(vcs, store, review, anchor, shown) {
        Ok(rounds::Placement::Whole) => (Value::Null, Value::Null),
        Ok(rounds::Placement::At(lines)) => (json!(lines), Value::Null),
        Ok(rounds::Placement::Drifted {
            last,
            made_on,
            lines,
        }) => (json!(last), json!({ "round": made_on, "lines": lines })),
        // The round it was made on can't be had here (its base is gone from this clone): where it was made
        Err(_) => (Value::Null, made()),
    };
    if let Some(map) = out.as_object_mut() {
        map.insert("placed".into(), at);
        map.insert("drifted".into(), drifted);
        map.insert("in_view".into(), Value::Bool(in_view));
    }
    out
}

/// A view, its chains placed in it, and the user's drafts (ATM-612): a draft comment placed as a chain on its anchor
/// is (one on what is there now, on the round shown, the newest), a reply as it is.
pub(super) fn shown_with<S: RoundDiffs + ?Sized>(
    vcs: &atm_vcs::GitVcs,
    diffs: &S,
    review: &Review,
    shown: u32,
    against: Option<u32>,
    drafts: &[Draft],
) -> Result<(Value, Vec<Value>, Vec<Value>)> {
    let view = view(vcs, diffs, review, shown, against)?;
    let chains = review
        .chains
        .iter()
        .map(|c| placed(vcs, diffs, review, c, shown, against))
        .collect();
    let newest = review.rounds.last().map_or(shown, |r| r.number);
    let drafts = drafts
        .iter()
        .map(|d| {
            let out = serde_json::to_value(d).unwrap_or(Value::Null);
            match &d.place {
                Place::Comment {
                    path,
                    lines,
                    side,
                    quote,
                    round,
                } => {
                    let anchor = Anchor {
                        round: round.unwrap_or(newest),
                        path: path.clone(),
                        side: *side,
                        lines: *lines,
                        quote: quote.clone(),
                    };
                    with_placement(vcs, diffs, review, &anchor, (shown, against), out)
                }
                Place::Reply { .. } => out,
            }
        })
        .collect();
    Ok((view, chains, drafts))
}

/// The user's drafts on `review` (ATM-612); none for an agent, or when they can't be read (the page shows the review
/// as it is either way).
pub(super) fn drafts_of(project: &Project, review: &Review) -> Vec<Draft> {
    crate::review_drafts::list_drafts(project, &review.header.reference).unwrap_or_default()
}
