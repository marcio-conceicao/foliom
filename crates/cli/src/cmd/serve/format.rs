//! Journal name ↔ date helpers (LNK-05, LNK-06).
//!
//! `format_journal_title(Date) -> String` produces the long-form English
//! title `"May 21st, 2026"` used in `/api/pages/:name` and
//! `/api/journals?from=...&to=...`. `parse_journal_name(&str) -> Option<Date>`
//! is the inverse for the `YYYY_MM_DD` filename stem Phase 1 produces.
//!
//! English ordinal logic (`1st/2nd/3rd/4th... 11th/12th/13th... 21st`) is
//! intentionally inlined rather than pulled from `time`'s formatter — the
//! `time` crate exposes neither ordinal suffixes nor long-form English month
//! names without enabling the `formatting` feature plus a custom format
//! description, and a 6-line `match` is dramatically clearer than a
//! `format_description!()` invocation here.

use time::{Date, Month};

/// Long-form English journal title: `"May 21st, 2026"`.
pub fn format_journal_title(date: Date) -> String {
    let day = date.day();
    let suffix = ordinal_suffix(day);
    let month = month_name(date.month());
    let year = date.year();
    format!("{} {}{}, {}", month, day, suffix, year)
}

/// English ordinal suffix for a day-of-month. `11/12/13` are "th" overrides
/// of the usual `n % 10` rule; everything else follows `1->st, 2->nd, 3->rd`.
fn ordinal_suffix(day: u8) -> &'static str {
    match day {
        11 | 12 | 13 => "th",
        n if n % 10 == 1 => "st",
        n if n % 10 == 2 => "nd",
        n if n % 10 == 3 => "rd",
        _ => "th",
    }
}

fn month_name(m: Month) -> &'static str {
    match m {
        Month::January => "January",
        Month::February => "February",
        Month::March => "March",
        Month::April => "April",
        Month::May => "May",
        Month::June => "June",
        Month::July => "July",
        Month::August => "August",
        Month::September => "September",
        Month::October => "October",
        Month::November => "November",
        Month::December => "December",
    }
}

/// Parse `YYYY_MM_DD` (the journal filename stem) into a `Date`.
///
/// Returns `None` if the input does not match exactly the underscored
/// 10-character shape or the date is not a real calendar date (e.g. Feb 30).
pub fn parse_journal_name(name: &str) -> Option<Date> {
    let parts: Vec<&str> = name.split('_').collect();
    if parts.len() != 3 {
        return None;
    }
    // Pin widths to avoid `_1_5` style fragments slipping through.
    if parts[0].len() != 4 || parts[1].len() != 2 || parts[2].len() != 2 {
        return None;
    }
    let y: i32 = parts[0].parse().ok()?;
    let m: u8 = parts[1].parse().ok()?;
    let d: u8 = parts[2].parse().ok()?;
    let month = Month::try_from(m).ok()?;
    Date::from_calendar_date(y, month, d).ok()
}

/// ISO `YYYY-MM-DD` formatter (does not require the `time/formatting` macro
/// feature). Used for `JournalEntry.date` in the range endpoint.
pub fn format_iso_date(date: Date) -> String {
    format!(
        "{:04}-{:02}-{:02}",
        date.year(),
        date.month() as u8,
        date.day()
    )
}

/// Inverse of `format_iso_date` — parses `YYYY-MM-DD`. Returns `None` on any
/// malformation. Used to validate `/api/journals?from=&to=` query params.
pub fn parse_iso_date(s: &str) -> Option<Date> {
    let parts: Vec<&str> = s.split('-').collect();
    if parts.len() != 3 {
        return None;
    }
    if parts[0].len() != 4 || parts[1].len() != 2 || parts[2].len() != 2 {
        return None;
    }
    let y: i32 = parts[0].parse().ok()?;
    let m: u8 = parts[1].parse().ok()?;
    let d: u8 = parts[2].parse().ok()?;
    let month = Month::try_from(m).ok()?;
    Date::from_calendar_date(y, month, d).ok()
}

/// Convert `YYYY-MM-DD` to `YYYY_MM_DD` (filename form) without re-parsing.
pub fn iso_to_filename(s: &str) -> Option<String> {
    let d = parse_iso_date(s)?;
    Some(format!(
        "{:04}_{:02}_{:02}",
        d.year(),
        d.month() as u8,
        d.day()
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use time::Month;

    fn d(y: i32, m: Month, day: u8) -> Date {
        Date::from_calendar_date(y, m, day).unwrap()
    }

    #[test]
    fn ordinal_st_nd_rd_th() {
        assert_eq!(format_journal_title(d(2026, Month::May, 1)), "May 1st, 2026");
        assert_eq!(format_journal_title(d(2026, Month::May, 2)), "May 2nd, 2026");
        assert_eq!(format_journal_title(d(2026, Month::May, 3)), "May 3rd, 2026");
        assert_eq!(format_journal_title(d(2026, Month::May, 4)), "May 4th, 2026");
    }

    #[test]
    fn teens_are_th_not_st_nd_rd() {
        assert_eq!(format_journal_title(d(2026, Month::May, 11)), "May 11th, 2026");
        assert_eq!(format_journal_title(d(2026, Month::May, 12)), "May 12th, 2026");
        assert_eq!(format_journal_title(d(2026, Month::May, 13)), "May 13th, 2026");
        assert_eq!(format_journal_title(d(2026, Month::May, 14)), "May 14th, 2026");
    }

    #[test]
    fn twenty_first_through_twenty_third() {
        assert_eq!(format_journal_title(d(2026, Month::May, 21)), "May 21st, 2026");
        assert_eq!(format_journal_title(d(2026, Month::May, 22)), "May 22nd, 2026");
        assert_eq!(format_journal_title(d(2026, Month::May, 23)), "May 23rd, 2026");
    }

    #[test]
    fn thirty_first() {
        assert_eq!(format_journal_title(d(2026, Month::May, 31)), "May 31st, 2026");
    }

    #[test]
    fn march_15_2024_canonical_example() {
        assert_eq!(format_journal_title(d(2024, Month::March, 15)), "March 15th, 2024");
    }

    #[test]
    fn parse_round_trip() {
        let date = parse_journal_name("2024_03_15").unwrap();
        assert_eq!(date.year(), 2024);
        assert_eq!(date.month() as u8, 3);
        assert_eq!(date.day(), 15);
    }

    #[test]
    fn parse_rejects_malformed() {
        assert!(parse_journal_name("2024-03-15").is_none()); // wrong separator
        assert!(parse_journal_name("2024_3_15").is_none()); // width
        assert!(parse_journal_name("2024_13_01").is_none()); // bad month
        assert!(parse_journal_name("not_a_date").is_none());
        assert!(parse_journal_name("").is_none());
    }

    #[test]
    fn iso_round_trip() {
        let d_ = d(2024, Month::March, 15);
        assert_eq!(format_iso_date(d_), "2024-03-15");
        assert_eq!(parse_iso_date("2024-03-15"), Some(d_));
        assert_eq!(iso_to_filename("2024-03-15").as_deref(), Some("2024_03_15"));
    }
}
