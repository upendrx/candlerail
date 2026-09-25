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

/// Day number (for daily resets such as session VWAP).
pub fn day(ms: i64) -> i64 {
    ms.div_euclid(86_400_000)
}

#[cfg(test)]
mod tests {
    use super::*;

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
    }
}
