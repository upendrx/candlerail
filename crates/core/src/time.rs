//! Tiny UTC date helpers, enough for "2024-01-31" style arguments and labels.

/// Days since 1970-01-01 for a civil date (Howard Hinnant's algorithm).
fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let doy = (153 * (m + if m > 2 { -3 } else { 9 }) + 2) / 5 + d - 1;
    era * 146_097 + yoe * 365 + yoe / 4 - yoe / 100 + doy - 719_468
}

fn civil_from_days(z: i64) -> (i64, i64, i64) {
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (if m <= 2 { y + 1 } else { y }, m, d)
}

/// Parses `YYYY-MM-DD`, `YYYY-MM-DD HH:MM`, `YYYY-MM-DDTHH:MM:SS` (UTC), or a
/// plain number of milliseconds (or seconds, if it's too small to be ms).
pub fn parse(s: &str) -> Option<i64> {
    let s = s.trim().trim_end_matches('Z');
    if let Ok(n) = s.parse::<i64>() {
        return Some(if n.abs() < 100_000_000_000 { n * 1000 } else { n });
    }
    let (date, clock) = match s.split_once(['T', ' ']) {
        Some((d, c)) => (d, c),
        None => (s, "00:00:00"),
    };
    let mut d = date.split('-').map(|x| x.parse::<i64>().ok());
    let (y, m, day) = (d.next()??, d.next()??, d.next()??);
    if !(1..=12).contains(&m) || !(1..=31).contains(&day) {
        return None;
    }
    let mut c = clock.split(':').map(|x| x.split('.').next().unwrap_or("0").parse::<i64>().ok());
    let h = c.next().flatten().unwrap_or(0);
    let mi = c.next().flatten().unwrap_or(0);
    let sec = c.next().flatten().unwrap_or(0);
    Some(((days_from_civil(y, m, day) * 24 + h) * 60 + mi) * 60_000 + sec * 1000)
}

/// `2024-01-31 14:00` in UTC.
pub fn format(ms: i64) -> String {
    let days = ms.div_euclid(86_400_000);
    let rem = ms.rem_euclid(86_400_000) / 60_000;
    let (y, m, d) = civil_from_days(days);
    format!("{y:04}-{m:02}-{d:02} {:02}:{:02}", rem / 60, rem % 60)
}

/// Months since January 1970, for calendar-month periods.
pub fn month_index(ms: i64) -> i64 {
    let (y, m, _) = civil_from_days(ms.div_euclid(86_400_000));
    (y - 1970) * 12 + (m - 1)
}

/// Day number (for daily resets such as session VWAP).
pub fn day(ms: i64) -> i64 {
    ms.div_euclid(86_400_000)
}

/// Weekday of a day number (see [`day`]), Monday = 0.
pub fn weekday(day: i64) -> i64 {
    (day + 3).rem_euclid(7)
}

/// The day number of the `n`th Sunday (1-based) of a month, or of the last
/// Sunday when `n` is 0.
fn sunday(y: i64, m: i64, n: i64) -> i64 {
    if n == 0 {
        let next = if m == 12 { days_from_civil(y + 1, 1, 1) } else { days_from_civil(y, m + 1, 1) };
        let last = next - 1;
        last - (weekday(last) + 1) % 7
    } else {
        let first = days_from_civil(y, m, 1);
        first + (6 - weekday(first)).rem_euclid(7) + 7 * (n - 1)
    }
}

/// Whether New York is on daylight saving time on a day: from the second
/// Sunday of March to the first Sunday of November.
pub fn us_dst(day: i64) -> bool {
    let (y, _, _) = civil_from_days(day);
    day >= sunday(y, 3, 2) && day < sunday(y, 11, 1)
}

/// Whether London is on summer time on a day: from the last Sunday of March
/// to the last Sunday of October.
pub fn uk_dst(day: i64) -> bool {
    let (y, _, _) = civil_from_days(day);
    day >= sunday(y, 3, 0) && day < sunday(y, 10, 0)
}

/// Stock-market sessions that crypto traders watch.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Session {
    /// New York, 09:30 local.
    NewYork,
    /// London, 08:00 local.
    London,
    /// Tokyo, 09:00 local (00:00 UTC; Japan has no daylight saving).
    Tokyo,
}

impl Session {
    pub fn from_code(code: u64) -> Option<Session> {
        match code {
            1 => Some(Session::NewYork),
            2 => Some(Session::London),
            3 => Some(Session::Tokyo),
            _ => None,
        }
    }

    /// The session's open on a day, in minutes after 00:00 UTC.
    pub fn open_minutes(self, day: i64) -> i64 {
        match self {
            Session::NewYork => {
                if us_dst(day) {
                    13 * 60 + 30
                } else {
                    14 * 60 + 30
                }
            }
            Session::London => {
                if uk_dst(day) {
                    7 * 60
                } else {
                    8 * 60
                }
            }
            Session::Tokyo => 0,
        }
    }
}

/// Key of the period containing `ts`: hours or days count from 1970, weeks
/// start on Monday, and 43200 minutes means calendar months.
pub fn period_key(minutes: u64, ts: i64) -> i64 {
    match minutes {
        43_200 => month_index(ts),
        // 1970-01-05 was a Monday: shift so weeks start on Monday.
        10_080 => (day(ts) - 4).div_euclid(7),
        m => ts.div_euclid(m.max(1) as i64 * 60_000),
    }
}

/// When the period with this key starts.
pub fn period_start(minutes: u64, key: i64) -> i64 {
    match minutes {
        43_200 => days_from_civil(1970 + key.div_euclid(12), key.rem_euclid(12) + 1, 1) * 86_400_000,
        10_080 => (key * 7 + 4) * 86_400_000,
        m => key * m.max(1) as i64 * 60_000,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn daylight_saving_dates() {
        let d = |t: &str| day(parse(t).unwrap());
        // 2026: US from 8 March to 1 November; UK from 29 March to 25 October.
        assert!(!us_dst(d("2026-03-07")) && us_dst(d("2026-03-08")));
        assert!(us_dst(d("2026-10-31")) && !us_dst(d("2026-11-01")));
        assert!(!uk_dst(d("2026-03-28")) && uk_dst(d("2026-03-29")));
        assert!(uk_dst(d("2026-10-24")) && !uk_dst(d("2026-10-25")));
        assert_eq!(Session::NewYork.open_minutes(d("2026-07-01")), 810);
        assert_eq!(Session::NewYork.open_minutes(d("2026-01-15")), 870);
        assert_eq!(Session::London.open_minutes(d("2026-01-15")), 480);
    }

    #[test]
    fn period_boundaries() {
        let t = parse("2026-09-23 10:00").unwrap(); // a Wednesday
        assert_eq!(format(period_start(10_080, period_key(10_080, t))), "2026-09-21 00:00");
        assert_eq!(format(period_start(43_200, period_key(43_200, t))), "2026-09-01 00:00");
        assert_eq!(format(period_start(1440, period_key(1440, t) + 1)), "2026-09-24 00:00");
    }

    #[test]
    fn roundtrip() {
        let t = parse("2024-02-29").unwrap();
        assert_eq!(t, 1_709_164_800_000);
        assert_eq!(format(t), "2024-02-29 00:00");
        assert_eq!(parse("2024-02-29T13:45:00Z").unwrap(), t + (13 * 60 + 45) * 60_000);
        assert_eq!(parse("1709164800"), Some(t));
        assert_eq!(parse("1709164800000"), Some(t));
        assert_eq!(parse("2024-13-01"), None);
        assert_eq!(parse("yesterday"), None);
        assert_eq!(month_index(parse("1970-01-31").unwrap()), 0);
        assert_eq!(month_index(parse("2024-02-29").unwrap()), 649);
    }
}
