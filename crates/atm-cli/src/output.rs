//! How atm prints for people (docs/cli-output-design.md, T-488): the same words to a terminal and to a pipe, with
//! styles and lines cut to the width only on a terminal. Styles follow `--color` (auto, always, never); `auto` is
//! colour on a terminal unless `NO_COLOR` is set or `TERM` is `dumb` (anstream's rules, as clap's help has them).
//! Agents run atm with standard output piped, so they get plain text, uncut, which the CLI suite checks.

use std::io::IsTerminal;
use std::sync::OnceLock;

use anstyle::{AnsiColor, Effects, Style};
use atm_common::vocabulary::{Kind, Vocabulary};

/// `--color`: when atm styles its output.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, clap::ValueEnum)]
pub enum ColorWhen {
    // No doc comments: clap would show them, and the help in its long form (the flag's own help names the values)
    #[default]
    Auto,
    Always,
    Never,
}

#[derive(Debug, Clone, Copy)]
struct Settings {
    colour: bool,
    width: Option<usize>,
}

static SETTINGS: OnceLock<Settings> = OnceLock::new();

/// Decide, once per process, whether to style and how wide the terminal is. `--json` is never styled.
pub fn init(when: ColorWhen, json: bool) {
    let choice = match when {
        ColorWhen::Auto => anstream::ColorChoice::Auto,
        ColorWhen::Always => anstream::ColorChoice::Always,
        ColorWhen::Never => anstream::ColorChoice::Never,
    };
    choice.write_global();
    let stdout = std::io::stdout();
    let colour = !json && anstream::AutoStream::choice(&stdout) != anstream::ColorChoice::Never;
    let width = if stdout.is_terminal() {
        terminal_size::terminal_size().map(|(w, _)| usize::from(w.0))
    } else {
        None
    };
    let _ = SETTINGS.set(Settings { colour, width });
}

fn settings() -> Settings {
    *SETTINGS.get_or_init(|| Settings {
        colour: false,
        width: None,
    })
}

/// The terminal's width; `None` in a pipe or a file, where nothing is cut.
#[must_use]
pub fn width() -> Option<usize> {
    settings().width
}

/// Print a line, styles and all (anstream turns them into the Windows console's own where it needs to).
pub fn emit(line: &str) {
    anstream::println!("{line}");
}

/// A style, applied when styling is on.
#[must_use]
pub fn paint(style: Style, text: &str) -> String {
    if settings().colour && !text.is_empty() && style != Style::new() {
        format!("{style}{text}{style:#}")
    } else {
        text.to_string()
    }
}

/// Section headings, a task's ID and name in `details`.
pub const HEADING: Style = Style::new().effects(Effects::BOLD);
/// Secondary text: labels, agents, timestamps, sizes, hints.
pub const DIM: Style = Style::new().effects(Effects::DIMMED);

/// `error:`, red and bold.
#[must_use]
pub fn error_prefix() -> String {
    paint(
        Style::new().fg_color(Some(AnsiColor::Red.into())).bold(),
        "error:",
    )
}

/// `warning:`, yellow and bold.
#[must_use]
pub fn warning_prefix() -> String {
    paint(
        Style::new().fg_color(Some(AnsiColor::Yellow.into())).bold(),
        "warning:",
    )
}

/// A status's style, by its kind: a project's own statuses get their kind's.
#[must_use]
pub fn kind_style(kind: Option<Kind>) -> Style {
    let colour = |c: AnsiColor| Style::new().fg_color(Some(c.into()));
    match kind {
        Some(Kind::Active) => colour(AnsiColor::Blue),
        Some(Kind::Review) => colour(AnsiColor::Magenta),
        Some(Kind::Blocked) => colour(AnsiColor::Yellow),
        Some(Kind::Done) => colour(AnsiColor::Green),
        Some(Kind::Skipped) => DIM,
        Some(Kind::Todo) | None => Style::new(),
    }
}

/// A priority's style, by its place in the project's list: the first red and bold, the second yellow, the last
/// dimmed (with three or more), the rest plain.
#[must_use]
pub fn priority_style(position: Option<usize>, count: usize) -> Style {
    match position {
        Some(0) if count > 1 => Style::new().fg_color(Some(AnsiColor::Red.into())).bold(),
        Some(1) if count > 2 => Style::new().fg_color(Some(AnsiColor::Yellow.into())),
        Some(p) if count > 2 && p + 1 == count => DIM,
        _ => Style::new(),
    }
}

/// A project's statuses and priorities, for their styles.
#[derive(Debug, Clone, Default)]
pub struct Palette {
    vocab: Option<Vocabulary>,
}

impl Palette {
    /// The palette of a project's task file (none when it can't be read: everything plain).
    #[must_use]
    pub fn of_document(document: &serde_json::Value) -> Self {
        Palette {
            vocab: Vocabulary::of_document(document).ok(),
        }
    }

    /// The palette of a project's vocabularies.
    #[must_use]
    pub fn of_vocab(vocab: Vocabulary) -> Self {
        Palette { vocab: Some(vocab) }
    }

    /// A status's style, by the kind its project gives it.
    #[must_use]
    pub fn status(&self, status: &str) -> Style {
        kind_style(
            self.vocab
                .as_ref()
                .and_then(|v| v.statuses.iter().find(|s| s.id == status).map(|s| s.kind)),
        )
    }

    /// A priority's style, by its place in the project's list.
    #[must_use]
    pub fn priority(&self, priority: &str) -> Style {
        self.vocab.as_ref().map_or_else(Style::new, |v| {
            priority_style(
                v.priorities.iter().position(|p| p.id == priority),
                v.priorities.len(),
            )
        })
    }
}

/// A table's cell: its text and its style.
#[derive(Debug, Clone, Default)]
pub struct Cell {
    text: String,
    style: Style,
}

impl Cell {
    /// A plain cell.
    #[must_use]
    pub fn new(text: impl Into<String>) -> Self {
        Cell {
            text: text.into(),
            style: Style::new(),
        }
    }

    /// A styled cell.
    #[must_use]
    pub fn styled(text: impl Into<String>, style: Style) -> Self {
        Cell {
            text: text.into(),
            style,
        }
    }
}

impl From<String> for Cell {
    fn from(text: String) -> Self {
        Cell::new(text)
    }
}

impl From<&str> for Cell {
    fn from(text: &str) -> Self {
        Cell::new(text)
    }
}

/// Rows of cells, each column padded to its widest value, two spaces apart; the last column not padded. On a
/// terminal one column (the name) is cut with `…` so the line fits.
#[derive(Debug, Clone, Default)]
pub struct Table {
    rows: Vec<Vec<Cell>>,
    cut: Option<usize>,
    indent: usize,
    right: Vec<usize>,
}

/// The narrowest a cut column gets.
const MIN_CUT: usize = 16;

impl Table {
    /// An empty table.
    #[must_use]
    pub fn new() -> Self {
        Table::default()
    }

    /// Cut this column (from 0) on a terminal, so the lines fit its width.
    #[must_use]
    pub fn cut(mut self, column: usize) -> Self {
        self.cut = Some(column);
        self
    }

    /// Align this column (from 0) to the right, as numbers are.
    #[must_use]
    pub fn right(mut self, column: usize) -> Self {
        self.right.push(column);
        self
    }

    /// Indent every line by this many spaces.
    #[must_use]
    pub fn indent(mut self, spaces: usize) -> Self {
        self.indent = spaces;
        self
    }

    /// Add a row.
    pub fn row(&mut self, cells: Vec<Cell>) {
        self.rows.push(cells);
    }

    /// The lines, for the terminal's width ([`width`]).
    #[must_use]
    pub fn lines(&self) -> Vec<String> {
        self.lines_at(width())
    }

    /// The lines, cut to `width` when there is one.
    #[must_use]
    pub fn lines_at(&self, width: Option<usize>) -> Vec<String> {
        let columns = self.rows.iter().map(Vec::len).max().unwrap_or(0);
        let mut widths: Vec<usize> = (0..columns)
            .map(|c| {
                self.rows
                    .iter()
                    .filter_map(|r| r.get(c))
                    .map(|cell| chars(&cell.text))
                    .max()
                    .unwrap_or(0)
            })
            .collect();
        // A column empty in every row takes no room
        let used: Vec<bool> = widths.iter().map(|w| *w > 0).collect();
        if let (Some(width), Some(cut)) = (width, self.cut.filter(|c| *c < columns)) {
            let others: usize = widths
                .iter()
                .enumerate()
                .filter(|(c, w)| *c != cut && **w > 0)
                .map(|(_, w)| w + 2)
                .sum();
            let room = width.saturating_sub(others + self.indent);
            if widths[cut] > room {
                widths[cut] = room.max(MIN_CUT);
            }
        }
        self.rows
            .iter()
            .map(|row| {
                let last = (0..row.len()).rev().find(|c| !row[*c].text.is_empty());
                let mut line = " ".repeat(self.indent);
                let mut first = true;
                for (c, cell) in row.iter().enumerate() {
                    if !used[c] || last.is_none_or(|l| c > l) {
                        continue;
                    }
                    if !first {
                        line.push_str("  ");
                    }
                    first = false;
                    let text = if Some(c) == self.cut {
                        cut_to(&cell.text, widths[c])
                    } else {
                        cell.text.clone()
                    };
                    let room = widths[c].saturating_sub(chars(&text));
                    if self.right.contains(&c) {
                        line.push_str(&" ".repeat(room));
                        line.push_str(&paint(cell.style, &text));
                    } else {
                        line.push_str(&paint(cell.style, &text));
                        if Some(c) != last {
                            line.push_str(&" ".repeat(room));
                        }
                    }
                }
                line
            })
            .collect()
    }
}

fn chars(text: &str) -> usize {
    text.chars().count()
}

/// `text` cut to `width` characters, the last one `…` when it was longer.
#[must_use]
pub fn cut_to(text: &str, width: usize) -> String {
    if chars(text) <= width {
        return text.to_string();
    }
    let mut cut: String = text.chars().take(width.saturating_sub(1)).collect();
    cut.push('…');
    cut
}

/// `text` wrapped at spaces to `width` (a word longer than that stays whole), or as it is without a width.
#[must_use]
pub fn wrap(text: &str, width: Option<usize>) -> Vec<String> {
    let Some(width) = width.filter(|w| *w > 0) else {
        return vec![text.to_string()];
    };
    if chars(text) <= width {
        return vec![text.to_string()];
    }
    let lead: String = text.chars().take_while(|c| *c == ' ').collect();
    let mut lines = Vec::new();
    let mut line = lead.clone();
    for word in text.split(' ').filter(|w| !w.is_empty()) {
        if chars(&line) > chars(&lead) && chars(&line) + 1 + chars(word) > width {
            lines.push(std::mem::replace(&mut line, lead.clone()));
        }
        if chars(&line) > chars(&lead) {
            line.push(' ');
        }
        line.push_str(word);
    }
    lines.push(line);
    lines
}

/// A section: a bold heading with its count, then its lines; nothing at all when it has none.
#[must_use]
pub fn section(title: &str, lines: Vec<String>, count: Option<usize>) -> Vec<String> {
    if lines.is_empty() {
        return Vec::new();
    }
    let title = count.map_or_else(|| title.to_string(), |n| format!("{title} ({n})"));
    let mut out = vec![String::new(), paint(HEADING, &title)];
    out.extend(lines);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn table() -> Table {
        let mut t = Table::new().cut(1);
        t.row(vec!["T-1".into(), "A short name".into(), "ux".into()]);
        t.row(vec![
            "T-10".into(),
            "A much longer name that goes on and on".into(),
            "".into(),
        ]);
        t
    }

    #[test]
    fn columns_are_padded_to_the_widest_and_the_last_is_not() {
        assert_eq!(
            table().lines_at(None),
            [
                "T-1   A short name                            ux",
                "T-10  A much longer name that goes on and on",
            ]
        );
    }

    #[test]
    fn on_a_terminal_the_cut_column_shrinks_to_fit_with_an_ellipsis() {
        let lines = table().lines_at(Some(30));
        assert_eq!(lines[0], "T-1   A short name          ux");
        assert_eq!(lines[1], "T-10  A much longer name …");
        assert!(lines.iter().all(|l| l.chars().count() <= 30));
    }

    #[test]
    fn a_cut_column_keeps_a_minimum_and_counts_characters_not_bytes() {
        assert_eq!(cut_to("“quoted” text…", 5), "“quo…");
        assert_eq!(chars(&cut_to("abcdefghijklmnopqrstuvwxyz", 16)), 16);
        let lines = table().lines_at(Some(5));
        assert!(lines[1].contains('…'));
    }

    #[test]
    fn empty_columns_take_no_room_and_indent_applies() {
        let mut t = Table::new().indent(2);
        t.row(vec!["a".into(), "".into(), "b".into()]);
        assert_eq!(t.lines_at(None), ["  a  b"]);
    }

    #[test]
    fn wrap_breaks_at_spaces_and_keeps_the_indent() {
        assert_eq!(
            wrap("    one two three four", Some(14)),
            ["    one two", "    three four"]
        );
        assert_eq!(wrap("one two", None), ["one two"]);
        assert_eq!(
            wrap("averyveryverylongword x", Some(5)),
            ["averyveryverylongword", "x"]
        );
    }

    #[test]
    fn an_empty_section_is_left_out() {
        assert_eq!(
            section("Blocked", Vec::new(), Some(0)),
            Vec::<String>::new()
        );
        assert_eq!(
            section("Blocked", vec!["  T-1".into()], Some(1)),
            ["", "Blocked (1)", "  T-1"]
        );
    }

    #[test]
    fn priorities_are_styled_by_their_place() {
        assert_ne!(priority_style(Some(0), 4), Style::new());
        assert_ne!(priority_style(Some(1), 4), Style::new());
        assert_eq!(priority_style(Some(2), 4), Style::new());
        assert_eq!(priority_style(Some(3), 4), DIM);
        assert_eq!(priority_style(None, 4), Style::new());
        assert_eq!(priority_style(Some(0), 1), Style::new());
    }

    #[test]
    fn nothing_is_styled_unless_styling_is_on() {
        // The tests never call init: styling stays off, as in a pipe
        assert_eq!(paint(HEADING, "x"), "x");
        assert_eq!(error_prefix(), "error:");
    }
}
