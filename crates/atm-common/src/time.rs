//! Time as atm writes it (the Python atm's `common/core.py`): seconds since 1970 in UTC, written as ISO 8601 to the
//! second (`2026-05-01T08:00:00Z`). `ATM_NOW` replaces the current time, for tests.

use std::time::{SystemTime, UNIX_EPOCH};

/// The current time in seconds since 1970 (UTC); `ATM_NOW` (ISO 8601) overrides it.
#[must_use]
pub fn now() -> i64 {
    std::env::var("ATM_NOW")
        .ok()
        .and_then(|t| parse_iso(&t))
        .unwrap_or_else(real_now)
}

/// The clock's time, whatever `ATM_NOW` says (how old a lock is).
#[must_use]
pub fn real_now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| i64::try_from(d.as_secs()).unwrap_or(i64::MAX))
}

/// The current time as atm writes it: `YYYY-MM-DDTHH:MM:SSZ`.
#[must_use]
pub fn now_iso() -> String {
    format_iso(now())
}

/// Days since 1970-01-01 for a date (Howard Hinnant's `days_from_civil`).
fn days_from_civil(year: i64, month: i64, day: i64) -> i64 {
    let y = if month <= 2 { year - 1 } else { year };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = (month + 9) % 12;
    let doy = (153 * mp + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// The date of a day since 1970-01-01 (`civil_from_days`): (year, month, day).
fn civil_from_days(days: i64) -> (i64, i64, i64) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    (yoe + era * 400 + i64::from(month <= 2), month, day)
}

/// Seconds since 1970 as `YYYY-MM-DDTHH:MM:SSZ`.
#[must_use]
pub fn format_iso(seconds: i64) -> String {
    let (days, rest) = (seconds.div_euclid(86_400), seconds.rem_euclid(86_400));
    let (year, month, day) = civil_from_days(days);
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}Z",
        rest / 3600,
        rest % 3600 / 60,
        rest % 60
    )
}

/// `YYYY-MM-DDTHH:MM:SSZ` as seconds since 1970, or None when the text isn't one (as Python's `strptime` refuses it).
#[must_use]
pub fn parse_iso(text: &str) -> Option<i64> {
    let b = text.as_bytes();
    let shape = b.len() == 20
        && b.iter().enumerate().all(|(i, c)| match i {
            4 | 7 => *c == b'-',
            10 => *c == b'T',
            13 | 16 => *c == b':',
            19 => *c == b'Z',
            _ => c.is_ascii_digit(),
        });
    if !shape {
        return None;
    }
    let num = |from: usize, to: usize| text[from..to].parse::<i64>().ok();
    let (year, month, day) = (num(0, 4)?, num(5, 7)?, num(8, 10)?);
    let (hour, minute, second) = (num(11, 13)?, num(14, 16)?, num(17, 19)?);
    let leap = (year % 4 == 0 && year % 100 != 0) || year % 400 == 0;
    let month_days = [
        31,
        if leap { 29 } else { 28 },
        31,
        30,
        31,
        30,
        31,
        31,
        30,
        31,
        30,
        31,
    ];
    let valid = (1..=12).contains(&month)
        && (1..=month_days[usize::try_from(month - 1).ok()?]).contains(&day)
        && hour < 24
        && minute < 60
        && second < 62;
    valid.then(|| days_from_civil(year, month, day) * 86_400 + hour * 3600 + minute * 60 + second)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn times_are_written_and_read_as_atm_writes_them() {
        assert_eq!(format_iso(0), "1970-01-01T00:00:00Z");
        assert_eq!(parse_iso("2026-05-01T08:30:00Z"), Some(1_777_624_200));
        assert_eq!(format_iso(1_777_624_200), "2026-05-01T08:30:00Z");
        assert_eq!(
            format_iso(parse_iso("2024-02-29T23:59:59Z").unwrap()),
            "2024-02-29T23:59:59Z"
        );
        assert_eq!(format_iso(-1), "1969-12-31T23:59:59Z");
    }

    #[test]
    fn a_text_that_isnt_a_time_is_none() {
        for bad in [
            "",
            "2026-05-01",
            "2026-05-01T08:30:00",
            "2026-13-01T00:00:00Z",
            "2025-02-29T00:00:00Z",
            "x026-05-01T08:30:00Z",
        ] {
            assert_eq!(parse_iso(bad), None, "{bad}");
        }
    }

    #[test]
    fn now_is_a_time() {
        assert!(now() > 1_700_000_000);
        assert!(parse_iso(&now_iso()).is_some());
    }
}
