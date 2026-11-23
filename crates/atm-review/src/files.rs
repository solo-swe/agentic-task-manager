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
mod tests;
