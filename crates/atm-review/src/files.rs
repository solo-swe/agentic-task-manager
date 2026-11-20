//! The two text files of a review (docs/code-review-design.md §3.3, §3.4), written and read back: `review.md`, the
//! header (TOML front matter) and the summary, and `comments.md`, every chain of comments. Text only, no files: the
//! file store (ATM-563) reads and writes them. People read and may edit both; what atm can't read back is kept as it is
//! and reported (for `atm doctor`), never dropped.

/// `review.md`: the header between `+++` lines, then the author's summary.
pub mod review_md {
    use atm_common::{Error, Result};
    use serde::{Deserialize, Serialize};
    use toml::value::Datetime;

    use crate::model::{Actor, ReviewHeader, ReviewId, ReviewRef, Status};

    /// The header as written: the dates as TOML dates, the reference left to the store (`CR-8.1` on a clash).
    #[derive(Serialize, Deserialize)]
    struct Header {
        id: String,
        number: u32,
        title: String,
        /// As the file writes it ([`stored`]): an older atm reads the review too
        status: String,
        #[serde(default)]
        tasks: Vec<String>,
        /// None (left out) while whose change it is isn't known
        #[serde(default, skip_serializing_if = "Option::is_none")]
        author: Option<String>,
        #[serde(default)]
        reviewers: Vec<String>,
        created: Datetime,
        updated: Datetime,
    }

    /// A status as `review.md` writes it: its name, but waiting on the reviewers, which the file keeps writing as
    /// `open`, as atm before 0.41.0 did and reads (ATM-613 renamed the status, not the file's word for it: a clone or
    /// an app on an older atm would otherwise not see the review at all)
    fn stored(status: Status) -> &'static str {
        match status {
            Status::Waiting => "open",
            other => other.name(),
        }
    }

    /// A status from `review.md`: `open` (as written) or `waiting` (as a 0.41.0 build wrote it a while) is waiting
    fn status_of(name: &str) -> Result<Status> {
        if name == "open" || name == "waiting" {
            return Ok(Status::Waiting);
        }
        Status::EVERY
            .into_iter()
            .find(|s| s.name() == name)
            .ok_or_else(|| {
                Error::Invalid(format!(
                    "review.md's header can't be read: no status {name}"
                ))
            })
    }

    fn date(text: &str, what: &str) -> Result<Datetime> {
        text.parse()
            .map_err(|e| Error::Invalid(format!("the review's {what} isn't a date ({text}): {e}")))
    }

    /// The file's text.
    ///
    /// # Errors
    /// A date in the header that isn't one.
    pub fn write(header: &ReviewHeader, summary: &str) -> Result<String> {
        let toml = toml::to_string(&Header {
            id: header.id.0.clone(),
            number: header.number,
            title: header.title.clone(),
            status: stored(header.status).to_string(),
            tasks: header.tasks.clone(),
            author: header.author.as_ref().map(|a| a.0.clone()),
            reviewers: header.reviewers.iter().map(|r| r.0.clone()).collect(),
            created: date(&header.created, "creation time")?,
            updated: date(&header.updated, "update time")?,
        })
        .map_err(|e| Error::failed(format!("the review's header: {e}")))?;
        let summary = summary.trim_end();
        Ok(if summary.is_empty() {
            format!("+++\n{toml}+++\n")
        } else {
            format!("+++\n{toml}+++\n\n{summary}\n")
        })
    }

    /// The header and the summary, from the file's text; the reference is `CR-<number>` (the store makes it
    /// `CR-<number>.<n>` when two reviews share the number).
    ///
    /// # Errors
    /// No header, or one that can't be read: the review can't be known without it.
    pub fn read(text: &str) -> Result<(ReviewHeader, String)> {
        let text = text.replace("\r\n", "\n");
        let rest = text.strip_prefix("+++\n").ok_or_else(|| {
            Error::Invalid("review.md doesn't start with its header (+++)".into())
        })?;
        let end = rest
            .find("\n+++\n")
            .map(|at| (at + 1, at + 5))
            .or_else(|| {
                rest.strip_suffix("\n+++")
                    .map(|head| (head.len() + 1, rest.len()))
            })
            .ok_or_else(|| Error::Invalid("review.md's header has no closing +++".into()))?;
        let header: Header = toml::from_str(&rest[..end.0])
            .map_err(|e| Error::Invalid(format!("review.md's header can't be read: {e}")))?;
        let after = &rest[end.1..];
        let summary = after
            .strip_prefix('\n')
            .unwrap_or(after)
            .trim_end()
            .to_string();
        Ok((
            ReviewHeader {
                id: ReviewId(header.id),
                reference: ReviewRef::numbered(header.number),
                number: header.number,
                title: header.title,
                status: status_of(&header.status)?,
                // A task by its key; one an earlier atm wrote as people read a provisional key (`~CMD-36`, ATM-594)
                // is the same key
                tasks: header
                    .tasks
                    .into_iter()
                    .map(|t| t.strip_prefix('~').map(str::to_string).unwrap_or(t))
                    .collect(),
                author: header.author.map(Actor),
                reviewers: header.reviewers.into_iter().map(Actor).collect(),
                created: header.created.to_string(),
                updated: header.updated.to_string(),
            },
            summary,
        ))
    }
}

/// `comments.md`: a `##` heading per chain (what it is about, the round, its ID), the quote, then its comments as a
/// nested list: a top-level comment starts with its resolved box (`[ ]`, `[x]`), a reply has none; then the author,
/// the time and the ID in bold and `·`s, and the text indented under it.
pub mod comments_md {
    use crate::model::{Actor, Anchor, Chain, ChainId, Comment, CommentId, Side};

    /// A line atm couldn't read, kept where it was: after the chain it followed (or at the top).
    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct Unread {
        /// The chain it came after, none at the top of the file.
        pub after: Option<ChainId>,
        /// Its line number in the file read (1-based).
        pub line: usize,
        /// The line, as it was.
        pub text: String,
    }

    /// A review's `comments.md`: its chains, and what couldn't be read.
    #[derive(Debug, Clone, PartialEq, Eq, Default)]
    pub struct CommentsFile {
        /// The chains, in the file's order.
        pub chains: Vec<Chain>,
        /// The lines atm couldn't read, kept.
        pub unread: Vec<Unread>,
    }

    impl CommentsFile {
        /// What `atm doctor` says about the file: a line per line it couldn't read.
        #[must_use]
        pub fn problems(&self) -> Vec<String> {
            self.unread
                .iter()
                .map(|u| {
                    format!(
                        "comments.md line {}: not a comment atm can read: {}",
                        u.line, u.text
                    )
                })
                .collect()
        }
    }

    const SEP: &str = " · ";
    const THE_REVIEW: &str = "(the review)";
    /// A poke's mark, where a top-level comment has its resolved box (ATM-611)
    const POKE: &str = "[poke] ";
    const OLD_SIDE: &str = "old side";

    fn heading(chain: &Chain) -> String {
        let anchor = &chain.anchor;
        let what = match &anchor.path {
            None => THE_REVIEW.to_string(),
            Some(path) => match anchor.lines {
                Some((first, last)) if first == last => format!("{path}:{first}"),
                Some((first, last)) => format!("{path}:{first}-{last}"),
                None => path.clone(),
            },
        };
        let side = if anchor.side == Side::Old {
            format!("{SEP}{OLD_SIDE}")
        } else {
            String::new()
        };
        format!(
            "## {what}{SEP}round {}{side}{SEP}{}",
            anchor.round, chain.id.0
        )
    }

    /// The mark before a body line that would read as a comment's first line (ATM-658).
    const ESCAPE: char = '\\';

    /// Whether a body line, at any indent and under any number of escape marks, has the shape of a comment's first
    /// line (`- **author** · at · cN`, with or without a box), which `read_item` would take for a reply or a comment
    /// by whoever it names.
    fn comment_shaped(line: &str) -> bool {
        read_item(line.trim_start_matches(' ').trim_start_matches(ESCAPE)).is_some()
    }

    /// A body line as it is written: one of a comment's shape gets an escape mark after its own indent, so it is
    /// read back as body, never as a comment (ATM-658, I31). Every other line is as it was, so older files read the same.
    fn escape_body_line(line: &str) -> String {
        if !comment_shaped(line) {
            return line.to_string();
        }
        let rest = line.trim_start_matches(' ');
        format!("{}{ESCAPE}{rest}", &line[..line.len() - rest.len()])
    }

    /// A body line as it was given: one escape mark less on a line of a comment's shape (see [`escape_body_line`]).
    fn unescape_body_line(line: &str) -> String {
        let rest = line.trim_start_matches(' ');
        match rest.strip_prefix(ESCAPE) {
            Some(unmarked) if comment_shaped(rest) => {
                format!("{}{unmarked}", &line[..line.len() - rest.len()])
            }
            _ => line.to_string(),
        }
    }

    fn write_comment(out: &mut Vec<String>, comment: &Comment, depth: usize) {
        let indent = "  ".repeat(depth);
        let resolved = match comment.resolved {
            Some(true) => "[x] ",
            Some(false) => "[ ] ",
            None if comment.poke => POKE,
            None => "",
        };
        out.push(format!(
            "{indent}- {resolved}**{}**{SEP}{}{SEP}{}",
            comment.author.0, comment.at, comment.id.0
        ));
        for line in comment.body.trim_end().lines() {
            out.push(if line.is_empty() {
                String::new()
            } else {
                format!("{indent}  {}", escape_body_line(line))
            });
        }
        for reply in &comment.replies {
            write_comment(out, reply, depth + 1);
        }
    }

    /// The file's text.
    #[must_use]
    pub fn write(file: &CommentsFile) -> String {
        fn unread_after(file: &CommentsFile, after: Option<&ChainId>) -> Vec<String> {
            file.unread
                .iter()
                .filter(|u| u.after.as_ref() == after)
                .map(|u| u.text.clone())
                .collect()
        }
        let mut out: Vec<String> = unread_after(file, None);
        for chain in &file.chains {
            if !out.is_empty() {
                out.push(String::new());
            }
            out.push(heading(chain));
            if let Some(quote) = &chain.anchor.quote {
                for line in quote.lines() {
                    out.push(if line.is_empty() {
                        ">".into()
                    } else {
                        format!("> {line}")
                    });
                }
                out.push(String::new());
            }
            for comment in &chain.comments {
                write_comment(&mut out, comment, 0);
            }
            out.extend(unread_after(file, Some(&chain.id)));
        }
        if out.is_empty() {
            return String::new();
        }
        out.join("\n") + "\n"
    }

    /// A heading's anchor and chain ID, if it is one atm wrote.
    fn read_heading(line: &str) -> Option<(Anchor, ChainId)> {
        let mut parts: Vec<&str> = line.strip_prefix("## ")?.split(SEP).collect();
        let id = parts.pop()?;
        if !id.starts_with('t') || id[1..].parse::<u32>().is_err() {
            return None;
        }
        let side = if parts.last() == Some(&OLD_SIDE) {
            parts.pop();
            Side::Old
        } else {
            Side::New
        };
        let round: u32 = parts.pop()?.strip_prefix("round ")?.parse().ok()?;
        let what = parts.join(SEP);
        let (path, lines) = if what == THE_REVIEW {
            (None, None)
        } else {
            let lines = what.rsplit_once(':').and_then(|(path, range)| {
                let (first, last) = range.split_once('-').unwrap_or((range, range));
                Some((
                    path.to_string(),
                    (first.parse::<u32>().ok()?, last.parse::<u32>().ok()?),
                ))
            });
            // Lines run forwards from line 1 (ATM-670): a heading with 5-3 or 0 is not one atm wrote, kept unread
            if lines
                .as_ref()
                .is_some_and(|(_, (first, last))| *first == 0 || last < first)
            {
                return None;
            }
            match lines {
                Some((path, lines)) => (Some(path), Some(lines)),
                None => (Some(what), None),
            }
        };
        if path.as_deref().is_some_and(str::is_empty) {
            return None;
        }
        Some((
            Anchor {
                round,
                path,
                side,
                lines,
                quote: None,
            },
            ChainId(id.to_string()),
        ))
    }

    /// A comment's first line: its depth and the comment, if it is one atm wrote.
    fn read_item(line: &str) -> Option<(usize, Comment)> {
        let spaces = line.len() - line.trim_start_matches(' ').len();
        if !spaces.is_multiple_of(2) {
            return None;
        }
        let rest = line[spaces..].strip_prefix("- ")?;
        let (resolved, poke, rest) = if let Some(rest) = rest.strip_prefix("[ ] ") {
            (Some(false), false, rest)
        } else if let Some(rest) = rest.strip_prefix("[x] ") {
            (Some(true), false, rest)
        } else if let Some(rest) = rest.strip_prefix(POKE) {
            (None, true, rest)
        } else {
            (None, false, rest)
        };
        let (author, rest) = rest.strip_prefix("**")?.split_once(&format!("**{SEP}"))?;
        let (at, id) = rest.rsplit_once(SEP)?;
        if author.is_empty()
            || at.is_empty()
            || !id.starts_with('c')
            || id[1..].parse::<u32>().is_err()
        {
            return None;
        }
        Some((
            spaces / 2,
            Comment {
                id: CommentId(id.to_string()),
                author: Actor(author.to_string()),
                at: at.to_string(),
                body: String::new(),
                resolved,
                poke,
                replies: Vec::new(),
            },
        ))
    }

    /// Nest a chain's comments by their depth, the first `depth` items under the comment before them.
    fn nest(
        items: &mut std::iter::Peekable<std::vec::IntoIter<(usize, Comment)>>,
        depth: usize,
    ) -> Vec<Comment> {
        let mut out = Vec::new();
        while let Some((d, _)) = items.peek() {
            if *d != depth {
                break;
            }
            let (_, mut comment) = items.next().expect("peeked");
            comment.replies = nest(items, depth + 1);
            out.push(comment);
        }
        out
    }

    /// One chain as it is read: its heading's anchor, its quote and its comments, flat with their depths.
    struct Reading {
        anchor: Anchor,
        id: ChainId,
        quote: Vec<String>,
        items: Vec<(usize, Comment)>,
    }

    impl Reading {
        fn finish(mut self) -> Chain {
            for (_, comment) in &mut self.items {
                comment.body = comment.body.trim_end().to_string();
            }
            if !self.quote.is_empty() {
                self.anchor.quote = Some(self.quote.join("\n"));
            }
            Chain {
                id: self.id,
                anchor: self.anchor,
                comments: nest(&mut self.items.into_iter().peekable(), 0),
            }
        }
    }

    /// The chains in a file's text, and the lines atm couldn't read (kept, with where they were).
    #[must_use]
    pub fn read(text: &str) -> CommentsFile {
        let text = text.replace("\r\n", "\n");
        let mut file = CommentsFile::default();
        let mut current: Option<Reading> = None;
        let mut last_chain: Option<ChainId> = None;
        for (number, line) in text.lines().enumerate().map(|(i, l)| (i + 1, l)) {
            let unread = |file: &mut CommentsFile, after: Option<ChainId>| {
                file.unread.push(Unread {
                    after,
                    line: number,
                    text: line.to_string(),
                });
            };
            if line.starts_with("## ") {
                if let Some(done) = current.take() {
                    file.chains.push(done.finish());
                }
                match read_heading(line) {
                    Some((anchor, id)) => {
                        last_chain = Some(id.clone());
                        current = Some(Reading {
                            anchor,
                            id,
                            quote: Vec::new(),
                            items: Vec::new(),
                        });
                    }
                    None => unread(&mut file, last_chain.clone()),
                }
                continue;
            }
            let Some(chain) = current.as_mut() else {
                if !line.is_empty() {
                    unread(&mut file, last_chain.clone());
                }
                continue;
            };
            if chain.items.is_empty() && (line == ">" || line.starts_with("> ")) {
                chain
                    .quote
                    .push(line.strip_prefix("> ").unwrap_or("").to_string());
                continue;
            }
            let previous_depth = chain.items.last().map(|(d, _)| *d);
            if let Some((depth, comment)) = read_item(line) {
                let fits = match previous_depth {
                    None => depth == 0,
                    Some(previous) => depth <= previous + 1,
                };
                // A top-level comment has its box (a poke, its mark instead), a reply neither
                let top_level = comment.resolved.is_some() || comment.poke;
                if fits && (depth == 0) == top_level {
                    chain.items.push((depth, comment));
                    continue;
                }
            }
            if let Some((depth, comment)) = chain.items.last_mut() {
                let indent = "  ".repeat(*depth + 1);
                if line.is_empty() {
                    comment.body.push('\n');
                    continue;
                }
                if let Some(body) = line.strip_prefix(&indent) {
                    comment.body.push_str(&unescape_body_line(body));
                    comment.body.push('\n');
                    continue;
                }
            } else if line.is_empty() {
                continue;
            }
            unread(&mut file, Some(chain.id.clone()));
        }
        if let Some(done) = current.take() {
            file.chains.push(done.finish());
        }
        file
    }
}

#[cfg(test)]
mod tests {
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
}
