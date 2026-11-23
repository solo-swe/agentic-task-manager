//! Stretches of time, in milliseconds since 1970: their union, one less another, the gaps between them.

/// A stretch of time, `[start, end)`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Interval {
    pub start: i64,
    pub end: i64,
}

impl Interval {
    /// How long it is.
    #[must_use]
    pub fn length(self) -> i64 {
        self.end - self.start
    }
}

/// The union of `intervals`: sorted, disjoint, touching ones joined; empty ones left out.
#[must_use]
pub fn merge(intervals: &[Interval]) -> Vec<Interval> {
    let mut sorted: Vec<Interval> = intervals
        .iter()
        .copied()
        .filter(|i| i.end > i.start)
        .collect();
    sorted.sort_by_key(|i| i.start);
    let mut out: Vec<Interval> = Vec::new();
    for iv in sorted {
        match out.last_mut() {
            Some(last) if iv.start <= last.end => last.end = last.end.max(iv.end),
            _ => out.push(iv),
        }
    }
    out
}

/// `a` less `b`, both sorted and disjoint.
#[must_use]
pub fn subtract(a: &[Interval], b: &[Interval]) -> Vec<Interval> {
    let mut out = Vec::new();
    for iv in a {
        let mut cursor = iv.start;
        for cut in b {
            if cut.end <= cursor || cut.start >= iv.end {
                continue;
            }
            if cut.start > cursor {
                out.push(Interval {
                    start: cursor,
                    end: cut.start,
                });
            }
            cursor = cursor.max(cut.end);
        }
        if cursor < iv.end {
            out.push(Interval {
                start: cursor,
                end: iv.end,
            });
        }
    }
    out
}

/// `intervals` cut to `[from, to]`, the empty ones left out.
#[must_use]
pub fn clip(intervals: &[Interval], from: i64, to: i64) -> Vec<Interval> {
    intervals
        .iter()
        .map(|i| Interval {
            start: i.start.max(from),
            end: i.end.min(to),
        })
        .filter(|i| i.end > i.start)
        .collect()
}

/// The time `intervals` cover within `[from, to]`.
#[must_use]
pub fn covered(intervals: &[Interval], from: i64, to: i64) -> i64 {
    clip(intervals, from, to).iter().map(|i| i.length()).sum()
}

/// The gaps of at least `min` in the union of `intervals`, within `[from, to]`.
#[must_use]
pub fn gaps(intervals: &[Interval], from: i64, to: i64, min: i64) -> Vec<Interval> {
    let mut out = Vec::new();
    let mut cursor = from;
    let mut sorted = intervals.to_vec();
    sorted.sort_by_key(|i| i.start);
    for iv in sorted {
        if iv.end <= cursor {
            continue;
        }
        if iv.start >= to {
            break;
        }
        if iv.start - cursor >= min {
            out.push(Interval {
                start: cursor,
                end: iv.start,
            });
        }
        cursor = cursor.max(iv.end);
        if cursor >= to {
            break;
        }
    }
    if to - cursor >= min {
        out.push(Interval {
            start: cursor,
            end: to,
        });
    }
    out
}

/// The middle value of `values` (the mean of the middle two for an even count), none without values.
#[must_use]
pub fn median(values: &[f64]) -> Option<f64> {
    if values.is_empty() {
        return None;
    }
    let mut sorted = values.to_vec();
    sorted.sort_by(f64::total_cmp);
    let mid = sorted.len() / 2;
    Some(if sorted.len() % 2 == 1 {
        sorted[mid]
    } else {
        f64::midpoint(sorted[mid - 1], sorted[mid])
    })
}
