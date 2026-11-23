use super::comments_md::{self, CommentsFile};
use super::review_md;
use crate::model::{Actor, ChainId, ReviewHeader, ReviewId, ReviewRef, Side, Status};

/// docs/code-review-design.md §3.4's example, as atm writes it.
const COMMENTS: &str = "\
## crates/atm-cli/src/process.rs:40-52 · round 1 · t4
> let shell = 0;

- [ ] **Hakan Serce** · 2026-11-17T03:31:00Z · c9
  On Windows this follows the idle process. Find the parent instead?
  - **claude:0f56a916** · 2026-11-17T03:40:00Z · c10
    Done in round 2: `shell_pid()`, past Git Bash's forked copy of itself.

    A second paragraph.
    - **Hakan Serce** · 2026-11-17T04:05:00Z · c12
      What if the parent's parent is a different shell?
- [x] **Hakan Serce** · 2026-11-17T03:33:00Z · c11
  Nit: the comment above it says \"launchers\".
  - a list in the text
  - **not** a reply

## docs/plan.md · round 2 · old side · t5
- [ ] **Hakan Serce** · 2026-11-17T04:00:00Z · c13
  The old wording was clearer.

## (the review) · round 1 · t1
- [ ] **Hakan Serce** · 2026-11-17T03:45:00Z · c14
  Overall fine; the two above first.
";

#[test]
fn the_designs_example_is_read_as_nested_chains_and_written_back_exactly() {
    let file = comments_md::read(COMMENTS);
    assert!(file.unread.is_empty(), "{:?}", file.unread);
    assert_eq!(file.chains.len(), 3);
    let t4 = &file.chains[0];
    assert_eq!(t4.id, ChainId("t4".into()));
    assert_eq!(
        t4.anchor.path.as_deref(),
        Some("crates/atm-cli/src/process.rs")
    );
    assert_eq!(t4.anchor.lines, Some((40, 52)));
    assert_eq!(t4.anchor.quote.as_deref(), Some("let shell = 0;"));
    assert_eq!(t4.comments.len(), 2);
    let c9 = &t4.comments[0];
    assert_eq!((c9.id.0.as_str(), c9.resolved), ("c9", Some(false)));
    assert_eq!(c9.replies[0].author, Actor::from("claude:0f56a916"));
    assert_eq!(c9.replies[0].resolved, None);
    assert_eq!(
        c9.replies[0].body,
        "Done in round 2: `shell_pid()`, past Git Bash's forked copy of itself.\n\nA second paragraph."
    );
    assert_eq!(c9.replies[0].replies[0].id.0, "c12");
    // A list in a comment's text isn't a reply: no author, time and ID
    assert!(
        t4.comments[1]
            .body
            .ends_with("- a list in the text\n- **not** a reply")
    );
    assert_eq!(file.chains[1].anchor.side, Side::Old);
    assert_eq!(file.chains[1].anchor.lines, None);
    assert_eq!(file.chains[2].anchor.path, None);
    assert_eq!(comments_md::write(&file), COMMENTS);
}

/// ATM-611: a poke is marked where a top-level comment has its box, and read back as a poke, with its replies
#[test]
fn a_poke_is_written_marked_and_read_back_as_one() {
    let text = "\
## (the review) · round 2 · t3
- [poke] **Hakan Serce** · 2026-11-19T10:00:00Z · c7
  Where are we?
  - **claude:0f56a916** · 2026-11-19T10:05:00Z · c8
    Round 3 is coming: two comments left.
- [poke] **Hakan Serce** · 2026-11-19T11:00:00Z · c9
";
    let file = comments_md::read(text);
    assert!(file.unread.is_empty(), "{:?}", file.unread);
    let [first, second] = &file.chains[0].comments[..] else {
        panic!("two pokes")
    };
    assert!(first.poke && second.poke);
    assert_eq!(
        (first.resolved, first.body.as_str()),
        (None, "Where are we?")
    );
    assert!(!first.replies[0].poke);
    assert_eq!(second.body, "");
    assert_eq!(comments_md::write(&file), text);
}

#[test]
fn what_atm_cant_read_is_kept_where_it_was_and_reported() {
    let broken = "\
A note someone put at the top.

## src/a.rs:3 · round 1 · t1
- [ ] **Hakan Serce** · 2026-11-17T03:31:00Z · c1
  Why?
written at the margin by hand
- **a reply without its box** · 2026-11-17T03:32:00Z · c2

## a heading without an ID
- [ ] **Hakan Serce** · 2026-11-17T03:33:00Z · c3
";
    let file = comments_md::read(broken);
    assert_eq!(file.chains.len(), 1);
    assert_eq!(file.chains[0].comments[0].body, "Why?");
    let lines: Vec<usize> = file.unread.iter().map(|u| u.line).collect();
    assert_eq!(lines, [1, 6, 7, 9, 10]);
    assert_eq!(file.unread[0].after, None);
    assert_eq!(file.unread[1].after, Some(ChainId("t1".into())));
    assert!(file.problems()[1].starts_with("comments.md line 6: not a comment atm can read"));
    // Written back, nothing is lost; read again, the same
    let again = comments_md::read(&comments_md::write(&file));
    assert_eq!(again.chains, file.chains);
    assert_eq!(
        again
            .unread
            .iter()
            .map(|u| u.text.clone())
            .collect::<Vec<_>>(),
        file.unread
            .iter()
            .map(|u| u.text.clone())
            .collect::<Vec<_>>()
    );
}

#[test]
fn windows_line_ends_read_the_same_and_an_empty_file_is_no_chains() {
    let file = comments_md::read(&COMMENTS.replace('\n', "\r\n"));
    assert_eq!(comments_md::write(&file), COMMENTS);
    assert_eq!(comments_md::read(""), CommentsFile::default());
    assert_eq!(comments_md::write(&CommentsFile::default()), "");
}

fn header() -> ReviewHeader {
    ReviewHeader {
        id: ReviewId("3fa1c0d2-0000-7000-8000-000000000000".into()),
        reference: ReviewRef::numbered(7),
        number: 7,
        title: "Claim hold on Windows".into(),
        status: Status::ChangesRequested,
        tasks: vec!["ATM-482".into()],
        author: Some(Actor::from("claude:0f56a916")),
        reviewers: vec![Actor::from("Hakan Serce")],
        created: "2026-11-17T03:20:00Z".into(),
        updated: "2026-11-17T04:02:00Z".into(),
    }
}

/// ATM-594: an earlier atm wrote a provisional key as people read it; it reads as the key, and is written so
#[test]
fn a_task_written_with_a_provisional_keys_mark_reads_as_its_key() {
    let text = review_md::write(&header(), "").unwrap().replace(
        "tasks = [\"ATM-482\"]",
        "tasks = [\"~CMD-36\", \"ATM-482\", \"T-1\"]",
    );
    let (read, _) = review_md::read(&text).unwrap();
    assert_eq!(read.tasks, ["CMD-36", "ATM-482", "T-1"]);
    let again = review_md::write(&read, "").unwrap();
    assert!(
        again.contains("tasks = [\"CMD-36\", \"ATM-482\", \"T-1\"]"),
        "{again}"
    );
}

#[test]
fn review_md_is_the_header_in_toml_then_the_summary_and_reads_back() {
    let text = review_md::write(
        &header(),
        "The watcher found its shell as pid 0 on Windows.\n",
    )
    .unwrap();
    assert_eq!(
        text,
        "+++\nid = \"3fa1c0d2-0000-7000-8000-000000000000\"\nnumber = 7\ntitle = \"Claim hold on Windows\"\n\
             status = \"changes_requested\"\ntasks = [\"ATM-482\"]\nauthor = \"claude:0f56a916\"\n\
             reviewers = [\"Hakan Serce\"]\ncreated = 2026-11-17T03:20:00Z\nupdated = 2026-11-17T04:02:00Z\n+++\n\n\
             The watcher found its shell as pid 0 on Windows.\n"
    );
    let (read, summary) = review_md::read(&text).unwrap();
    assert_eq!(read, header());
    assert_eq!(summary, "The watcher found its shell as pid 0 on Windows.");
    assert_eq!(review_md::write(&read, &summary).unwrap(), text);
    // No summary: the header alone
    let bare = review_md::write(&header(), "").unwrap();
    assert!(bare.ends_with("+++\n"));
    assert_eq!(review_md::read(&bare).unwrap().1, "");
    // No author yet (a live review a person opened): the line left out, read back as none
    let unknown = ReviewHeader {
        author: None,
        ..header()
    };
    let text = review_md::write(&unknown, "").unwrap();
    assert!(!text.contains("author"), "{text}");
    assert_eq!(review_md::read(&text).unwrap().0.author, None);
}

#[test]
fn review_md_reads_windows_line_ends_a_header_closing_the_file_and_missing_lists() {
    let text = review_md::write(&header(), "Line one.\n\nLine two.  \n\n").unwrap();
    assert!(text.ends_with("+++\n\nLine one.\n\nLine two.\n"), "{text}");
    let (read, summary) = review_md::read(&text.replace('\n', "\r\n")).unwrap();
    assert_eq!(
        (read, summary.as_str()),
        (header(), "Line one.\n\nLine two.")
    );
    // The closing +++ as the file's last line, without its line end
    let bare = review_md::write(&header(), "").unwrap();
    let (read, summary) = review_md::read(bare.trim_end()).unwrap();
    assert_eq!((read, summary.as_str()), (header(), ""));
    // Written by hand: no tasks, reviewers or author lines are none
    let by_hand = "+++\nid = \"u\"\nnumber = 3\ntitle = \"T\"\nstatus = \"draft\"\n\
                       created = 2026-11-17T03:20:00Z\nupdated = 2026-11-17T03:20:00Z\n+++\n";
    let (read, _) = review_md::read(by_hand).unwrap();
    assert_eq!(
        (
            read.reference,
            read.tasks.len(),
            read.reviewers.len(),
            read.author
        ),
        (ReviewRef::numbered(3), 0, 0, None)
    );
    // A summary that starts without the blank line still reads
    let tight = format!("{bare}Summary.\n");
    assert_eq!(review_md::read(&tight).unwrap().1, "Summary.");
}

/// ATM-613: waiting on the reviewers is written `open`, as atm before 0.41.0 wrote and reads it, so an app or a
/// clone on an older atm still sees the review; `waiting`, as a 0.41.0 build wrote it a while, reads the same
#[test]
fn waiting_is_written_open_for_an_older_atm_and_both_read_as_waiting() {
    let mut waiting = header();
    waiting.status = Status::Waiting;
    let text = review_md::write(&waiting, "").unwrap();
    assert!(text.contains("status = \"open\"\n"), "{text}");
    assert_eq!(review_md::read(&text).unwrap().0.status, Status::Waiting);
    let newer = text.replace("status = \"open\"", "status = \"waiting\"");
    assert_eq!(review_md::read(&newer).unwrap().0.status, Status::Waiting);
    // Every other status by its own name, both ways
    for status in Status::EVERY.into_iter().filter(|s| *s != Status::Waiting) {
        let mut h = header();
        h.status = status;
        let text = review_md::write(&h, "").unwrap();
        assert!(
            text.contains(&format!("status = \"{}\"\n", status.name())),
            "{text}"
        );
        assert_eq!(review_md::read(&text).unwrap().0.status, status);
    }
}

#[test]
fn a_review_md_with_a_status_or_date_atm_doesnt_know_cant_be_read() {
    for header in [
        "id = \"u\"\nnumber = 1\ntitle = \"T\"\nstatus = \"merged\"\ncreated = 2026-11-17T03:20:00Z\nupdated = 2026-11-17T03:20:00Z\n",
        "id = \"u\"\nnumber = 1\ntitle = \"T\"\nstatus = \"open\"\ncreated = \"yesterday\"\nupdated = 2026-11-17T03:20:00Z\n",
        "id = \"u\"\nnumber = -1\ntitle = \"T\"\nstatus = \"open\"\ncreated = 2026-11-17T03:20:00Z\nupdated = 2026-11-17T03:20:00Z\n",
        "id = \"u\"\ntitle = \"T\"\nstatus = \"open\"\ncreated = 2026-11-17T03:20:00Z\nupdated = 2026-11-17T03:20:00Z\n",
    ] {
        let read = review_md::read(&format!("+++\n{header}+++\n"));
        assert!(
            read.as_ref()
                .is_err_and(|e| e.to_string().contains("can't be read")),
            "{header}: {read:?}"
        );
    }
    let mut bad = header();
    bad.updated = "later".into();
    assert!(
        review_md::write(&bad, "")
            .unwrap_err()
            .to_string()
            .contains("update time")
    );
}

#[test]
fn a_heading_names_a_line_a_range_a_whole_file_or_the_review_on_either_side() {
    use crate::model::{Anchor, Chain};
    for (path, lines, side, heading) in [
        (
            Some("a.rs"),
            Some((3, 3)),
            Side::New,
            "## a.rs:3 · round 2 · t1",
        ),
        (
            Some("a.rs"),
            Some((3, 5)),
            Side::Old,
            "## a.rs:3-5 · round 2 · old side · t1",
        ),
        (
            Some("docs/plan.md"),
            None,
            Side::New,
            "## docs/plan.md · round 2 · t1",
        ),
        (None, None, Side::New, "## (the review) · round 2 · t1"),
        // A path with a colon of its own, and with the separator in it, reads back the same
        (
            Some("a:b.rs"),
            Some((1, 2)),
            Side::New,
            "## a:b.rs:1-2 · round 2 · t1",
        ),
        (
            Some("odd · name.md"),
            None,
            Side::New,
            "## odd · name.md · round 2 · t1",
        ),
    ] {
        let chain = Chain {
            id: ChainId("t1".into()),
            anchor: Anchor {
                round: 2,
                path: path.map(String::from),
                side,
                lines,
                quote: None,
            },
            comments: Vec::new(),
        };
        let file = CommentsFile {
            chains: vec![chain.clone()],
            unread: Vec::new(),
        };
        let text = comments_md::write(&file);
        assert_eq!(text, format!("{heading}\n"));
        assert_eq!(comments_md::read(&text).chains, [chain], "{heading}");
    }
}

#[test]
fn a_heading_atm_didnt_write_is_kept_unread_with_what_follows_it() {
    for heading in [
        "## a.rs · round x · t1",
        "## a.rs · t1",
        "## a.rs · round 1 · tx",
        "## a.rs · round 1 · c1",
        "## :3 · round 1 · t1",
        "## t1",
        "## ",
        // Lines that don't run forwards from line 1 (ATM-670)
        "## a.rs:5-3 · round 1 · t1",
        "## a.rs:0 · round 1 · t1",
    ] {
        let text =
            format!("{heading}\n- [ ] **Hakan Serce** · 2026-11-17T03:31:00Z · c1\n  Why?\n");
        let file = comments_md::read(&text);
        assert!(file.chains.is_empty(), "{heading}: {:?}", file.chains);
        assert_eq!(file.unread.len(), 3, "{heading}");
        assert_eq!(comments_md::write(&file), text, "{heading}");
    }
}

#[test]
fn a_quote_keeps_its_blank_lines_and_ends_at_the_first_comment() {
    let text = "\
## a.rs:1-3 · round 1 · t1
> one
>
> three

- [ ] **Hakan Serce** · 2026-11-17T03:31:00Z · c1
  Why?
> not a quote: the comment's
";
    let file = comments_md::read(text);
    let chain = &file.chains[0];
    assert_eq!(chain.anchor.quote.as_deref(), Some("one\n\nthree"));
    // A line like a quote after a comment is no quote, and not the comment's text (not indented): kept unread
    assert_eq!(chain.comments[0].body, "Why?");
    assert_eq!(file.unread.len(), 1);
    assert_eq!(file.unread[0].line, 8);
    assert_eq!(comments_md::write(&file), text);
}

#[test]
fn a_comment_line_atm_didnt_write_is_text_or_kept_unread_never_a_comment() {
    let item = |line: &str| {
        format!(
            "## a.rs · round 1 · t1\n- [ ] **Hakan Serce** · 2026-11-17T03:31:00Z · c1\n  Why?\n{line}\n"
        )
    };
    // Not a comment of its own: under the comment before, indented, it is that comment's text
    for (line, body) in [
        // A reply two levels down at once
        (
            "    - **Ana** · 2026-11-17T03:32:00Z · c2",
            "Why?\n  - **Ana** · 2026-11-17T03:32:00Z · c2",
        ),
        // A reply with a box
        (
            "  - [ ] **Ana** · 2026-11-17T03:32:00Z · c2",
            "Why?\n- [ ] **Ana** · 2026-11-17T03:32:00Z · c2",
        ),
        // No ID, a bad ID, no author, no time
        (
            "  - **Ana** · 2026-11-17T03:32:00Z",
            "Why?\n- **Ana** · 2026-11-17T03:32:00Z",
        ),
        (
            "  - **Ana** · 2026-11-17T03:32:00Z · cx",
            "Why?\n- **Ana** · 2026-11-17T03:32:00Z · cx",
        ),
        (
            "  - **** · 2026-11-17T03:32:00Z · c2",
            "Why?\n- **** · 2026-11-17T03:32:00Z · c2",
        ),
        ("  - **Ana** ·  · c2", "Why?\n- **Ana** ·  · c2"),
    ] {
        let file = comments_md::read(&item(line));
        assert!(file.unread.is_empty(), "{line}: {:?}", file.unread);
        let comments = &file.chains[0].comments;
        assert_eq!(comments.len(), 1, "{line}");
        assert!(comments[0].replies.is_empty(), "{line}");
        assert_eq!(comments[0].body, body, "{line}");
    }
    // Not indented under it, and not a comment atm writes: kept unread
    for line in [
        " - **Ana** · 2026-11-17T03:32:00Z · c2",
        "- **Ana** · 2026-11-17T03:32:00Z · c2",
        "- [x] **Ana** · 2026-11-17T03:32:00Z",
        "text at the margin",
    ] {
        let text = item(line);
        let file = comments_md::read(&text);
        assert_eq!(file.unread.len(), 1, "{line}");
        assert_eq!(file.unread[0].text, line);
        assert_eq!(file.chains[0].comments.len(), 1, "{line}");
        assert_eq!(comments_md::write(&file), text, "{line}");
    }
}

#[test]
fn lines_before_the_first_comment_of_a_chain_are_kept_unread_and_blank_ones_dropped() {
    let text = "\n\n## a.rs · round 1 · t1\n\nA note before the comments.\n- [ ] **Hakan Serce** · 2026-11-17T03:31:00Z · c1\n  Why?\n";
    let file = comments_md::read(text);
    assert_eq!(file.chains[0].comments.len(), 1);
    assert_eq!(
        file.unread
            .iter()
            .map(|u| (u.line, u.after.clone()))
            .collect::<Vec<_>>(),
        [(5, Some(ChainId("t1".into())))]
    );
    assert_eq!(
        file.problems(),
        ["comments.md line 5: not a comment atm can read: A note before the comments."]
    );
}

/// ATM-658: a body line shaped like a comment's first line, or like a heading, is the body's, read back as it was
/// given, never a reply or a comment by whoever it names
#[test]
fn a_body_with_comment_and_heading_shaped_lines_round_trips_as_one_comments_body() {
    let reply = "- **Hakan Serce** · 2026-11-17T04:00:00Z · c99";
    let body = [
        "Looks fine.",
        reply,
        "    - [x] **Hakan Serce** · 2026-11-17T04:00:00Z · c98",
        "- [poke] **Hakan Serce** · 2026-11-17T04:00:00Z · c96",
        "\\- **Hakan Serce** · 2026-11-17T04:00:00Z · c97",
        "\\\\- **Hakan Serce** · 2026-11-17T04:00:00Z · c95",
        "## a.rs · round 1 · t9",
        "\\not a comment, kept as it is",
        "- **bold** text in a list",
    ]
    .join("\n");
    let mut file = comments_md::read(
        "## a.rs · round 1 · t1\n- [ ] **claude:0f56a916** · 2026-11-17T03:31:00Z · c1\n",
    );
    file.chains[0].comments[0].body = body.clone();
    let text = comments_md::write(&file);
    // Each comment-shaped line gets one mark more; the rest is written as given
    assert!(text.contains(&format!("  \\{reply}\n")), "{text}");
    assert!(text.contains("      \\- [x] **Hakan Serce**"), "{text}");
    assert!(
        text.contains("  \\\\- **Hakan Serce** · 2026-11-17T04:00:00Z · c97\n"),
        "{text}"
    );
    assert!(
        text.contains("  \\not a comment, kept as it is\n"),
        "{text}"
    );
    assert!(text.contains("  - **bold** text in a list\n"), "{text}");

    let back = comments_md::read(&text);
    assert!(back.unread.is_empty(), "{:?}", back.unread);
    assert_eq!(back.chains.len(), 1, "no chain of the heading in the body");
    let comment = &back.chains[0].comments[0];
    assert!(
        comment.replies.is_empty(),
        "no reply from the body: {:?}",
        comment.replies
    );
    assert_eq!(comment.body, body);
    assert_eq!(comments_md::write(&back), text);
}

/// ATM-658: an older file's body reads as before: a line it never escaped keeps its marks
#[test]
fn an_older_files_body_lines_with_marks_read_as_they_were() {
    let text = "\
## a.rs · round 1 · t1
- [ ] **Hakan Serce** · 2026-11-17T03:31:00Z · c1
  \\*not emphasis\\*
  \\\\server\\share
";
    let file = comments_md::read(text);
    assert_eq!(
        file.chains[0].comments[0].body,
        "\\*not emphasis\\*\n\\\\server\\share"
    );
    assert_eq!(comments_md::write(&file), text);
}

#[test]
fn a_comment_without_text_and_a_chain_without_comments_read_back() {
    let text = "\
## a.rs · round 1 · t1
- [x] **Hakan Serce** · 2026-11-17T03:31:00Z · c1

## b.rs · round 1 · t2
";
    let file = comments_md::read(text);
    assert!(file.unread.is_empty(), "{:?}", file.unread);
    assert_eq!(file.chains[0].comments[0].body, "");
    assert_eq!(file.chains[0].comments[0].resolved, Some(true));
    assert_eq!(file.chains[1].comments.len(), 0);
    assert_eq!(comments_md::write(&file), text);
}

#[test]
fn a_review_md_without_a_readable_header_is_refused_naming_why() {
    assert!(
        review_md::read("no header")
            .unwrap_err()
            .to_string()
            .contains("doesn't start")
    );
    assert!(
        review_md::read("+++\nid = 1\n")
            .unwrap_err()
            .to_string()
            .contains("no closing")
    );
    let wrong = review_md::read("+++\nid = 1\n+++\n")
        .unwrap_err()
        .to_string();
    assert!(wrong.contains("can't be read"), "{wrong}");
    let mut bad = header();
    bad.created = "yesterday".into();
    assert!(review_md::write(&bad, "").is_err());
}
