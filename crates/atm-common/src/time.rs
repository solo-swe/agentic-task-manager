//! Time as atm writes it (the Python atm's `common/core.py`): seconds since 1970 in UTC, written as ISO 8601 to the
//! second (`2026-05-01T08:00:00Z`). `ATM_NOW` replaces the current time, for tests, in debug builds only.

use std::time::{SystemTime, UNIX_EPOCH};

/// Whether `ATM_NOW` may replace the clock: in a debug build (the CLI suite's and the e2e suite's binaries) only,
/// never in a release (ATM-665/666, audit C4: `ATM_NOW=2099-… atm loop next --claim` took every other agent's
/// task, their leases run out by that clock). A second variable would be no barrier: whoever sets one sets both.
pub const CLOCK_OVERRIDE: bool = cfg!(debug_assertions);

/// The current time in seconds since 1970 (UTC); in a debug build, `ATM_NOW` (ISO 8601) overrides it.
#[must_use]
pub fn now() -> i64 {
    if CLOCK_OVERRIDE && let Some(at) = std::env::var("ATM_NOW").ok().and_then(|t| parse_iso(&t)) {
        return at;
    }
    real_now()
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

/// atm's clock ([`now`]) in the local time zone (`TZ`, else the system's), as seconds since 1970 shifted by the zone's
/// offset at that moment: the calendar fields of [`civil`] are then the local ones. UTC when the zone can't be read.
#[must_use]
pub fn local_now() -> i64 {
    let now = now();
    let offset = jiff::Timestamp::from_second(now)
        .map_or(0, |at| jiff::tz::TimeZone::system().to_offset(at).seconds());
    now + i64::from(offset)
}

/// A `YYYY-MM-DD` date as days since 1970-01-01, or None when the text isn't a date.
fn date_days(date: &str) -> Option<i64> {
    let b = date.as_bytes();
    let shape = b.len() == 10
        && b.iter().enumerate().all(|(i, c)| {
            if i == 4 || i == 7 {
                *c == b'-'
            } else {
                c.is_ascii_digit()
            }
        });
    if !shape {
        return None;
    }
    let (year, month, day) = (
        date[0..4].parse().ok()?,
        date[5..7].parse().ok()?,
        date[8..10].parse().ok()?,
    );
    if !(1..=12).contains(&month) || !(1..=31).contains(&day) {
        return None;
    }
    let days = days_from_civil(year, month, day);
    // A day the month doesn't have (2026-02-30) isn't one
    (civil_from_days(days) == (year, month, day)).then_some(days)
}

/// The one due-date rule (ATM-720): the days from the project's day (today, local, by atm's clock) to `date`
/// (`YYYY-MM-DD`): 0 on the day, 1 the day before, negative once it has passed; None for a text that isn't a date.
#[must_use]
pub fn days_until(date: &str) -> Option<i64> {
    Some(date_days(date)? - local_now().div_euclid(86_400))
}

/// When a due date (`YYYY-MM-DD`) has passed: the last second of that day in the local time zone, as atm writes a time;
/// None for a text that isn't a date.
#[must_use]
pub fn end_of_day(date: &str) -> Option<String> {
    date_days(date)?;
    let day: jiff::civil::Date = date.parse().ok()?;
    let end = day
        .at(23, 59, 59, 0)
        .to_zoned(jiff::tz::TimeZone::system())
        .ok()?;
    Some(format_iso(end.timestamp().as_second()))
}

/// A time's calendar fields: year, month (1–12), day, hour, minute.
#[must_use]
pub fn civil(seconds: i64) -> (i64, i64, i64, i64, i64) {
    let (days, rest) = (seconds.div_euclid(86_400), seconds.rem_euclid(86_400));
    let (year, month, day) = civil_from_days(days);
    (year, month, day, rest / 3600, rest % 3600 / 60)
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
        // No leap second: 23:59:60 would be the next day's 00:00:00 written another way (ATM-666, audit C16)
        && second < 60;
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
    fn a_due_date_counts_days_from_the_local_day_today_zero_yesterday_minus_one() {
        let (year, month, day, _, _) = civil(local_now());
        let today = format!("{year:04}-{month:02}-{day:02}");
        assert_eq!(days_until(&today), Some(0));
        let shifted = |by: i64| {
            let (y, m, d) = civil_from_days(date_days(&today).unwrap() + by);
            format!("{y:04}-{m:02}-{d:02}")
        };
        assert_eq!(days_until(&shifted(1)), Some(1));
        assert_eq!(days_until(&shifted(-1)), Some(-1));
        assert_eq!(days_until(&shifted(30)), Some(30));
        for bad in [
            "",
            "2026-5-01",
            "2026-02-30",
            "2026-13-01",
            "12 April",
            "2026-04-12T00:00:00Z",
        ] {
            assert_eq!(days_until(bad), None, "{bad}");
            assert_eq!(end_of_day(bad), None, "{bad}");
        }
    }

    #[test]
    fn a_due_date_ends_at_the_last_second_of_its_local_day() {
        // In local time it is 23:59:59 that day, with that day's offset (a summer and a winter date)
        for date in ["2026-04-08", "2026-12-24"] {
            let end = parse_iso(&end_of_day(date).unwrap()).unwrap();
            let local = jiff::Timestamp::from_second(end)
                .unwrap()
                .to_zoned(jiff::tz::TimeZone::system())
                .datetime();
            assert_eq!(local.to_string(), format!("{date}T23:59:59"));
        }
    }

    /// ATM-666 (audit C4): `ATM_NOW` moved the clock in every build, so `ATM_NOW=2099-… atm loop next --claim` took
    /// every other agent's task; now only a debug build (the tests' binaries) heeds it
    #[test]
    fn atm_now_moves_the_clock_in_a_debug_build_only() {
        assert_eq!(super::CLOCK_OVERRIDE, cfg!(debug_assertions));
    }

    #[test]
    fn a_time_is_read_up_to_each_fields_limit_and_refused_past_it() {
        for (text, read) in [
            ("2026-01-01T23:59:59Z", true),
            ("2026-01-01T24:00:00Z", false),
            ("2026-01-01T00:60:00Z", false),
            // No leap second (ATM-666): 23:59:60 would be the next day's midnight written another way
            ("2026-01-01T00:00:59Z", true),
            ("2026-01-01T23:59:60Z", false),
            ("2026-01-01T00:00:61Z", false),
            ("2026-01-00T00:00:00Z", false),
            ("2026-04-30T00:00:00Z", true),
            ("2026-04-31T00:00:00Z", false),
            ("2026-00-01T00:00:00Z", false),
            ("2000-02-29T00:00:00Z", true),
            ("1900-02-29T00:00:00Z", false),
            ("2026-05-01 08:30:00Z", false),
            ("2026-05-01T08:30:00z", false),
            ("2026-05-01T08:30:00Z ", false),
            ("+026-05-01T08:30:00Z", false),
        ] {
            assert_eq!(parse_iso(text).is_some(), read, "{text}");
        }
    }

    #[test]
    fn civil_fields_and_written_times_count_back_before_1970() {
        assert_eq!(civil(-60), (1969, 12, 31, 23, 59));
        assert_eq!(civil(0), (1970, 1, 1, 0, 0));
        assert_eq!(
            parse_iso("1960-03-01T12:00:00Z").map(format_iso).as_deref(),
            Some("1960-03-01T12:00:00Z")
        );
    }

    #[test]
    fn local_time_is_now_shifted_by_a_zone_offset() {
        // local_now reads the clock itself, so the second may turn between the reads: its shift from one of the
        // instants around it is the zone's offset (CI's Windows runner saw -1 once)
        let before = now();
        let local = local_now();
        let after = now();
        assert!(
            (before..=after)
                .map(|at| local - at)
                .any(|shift| shift.abs() <= 14 * 3600 && shift % 900 == 0),
            "{local} against {before}..={after}"
        );
        assert_eq!(
            civil(parse_iso("2026-05-12T22:38:00Z").unwrap()),
            (2026, 5, 12, 22, 38)
        );
    }

    #[test]
    fn now_is_a_time() {
        assert!(now() > 1_700_000_000);
        assert!(parse_iso(&now_iso()).is_some());
    }
}
