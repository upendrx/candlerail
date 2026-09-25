//! Candles from CSV files.
//!
//! The header decides the columns, so files from most sources load as-is. It
//! needs a time column (`time`, `timestamp`, `date`, `datetime` or `open_time`)
//! and `open`, `high`, `low`, `close`; `volume` is optional. Times can be Unix
//! milliseconds, Unix seconds, or dates like `2024-01-31` / `2024-01-31 14:00`
//! (UTC). Commas or semicolons both work.

use anyhow::{Context, bail};
use candlerail_core::{Candle, time};
use std::path::Path;

pub fn load(path: &Path) -> anyhow::Result<Vec<Candle>> {
    let text = std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
    parse(&text).with_context(|| format!("in {}", path.display()))
}

pub fn parse(text: &str) -> anyhow::Result<Vec<Candle>> {
    let mut lines = text.lines().filter(|l| !l.trim().is_empty());
    let header = lines.next().context("empty file")?;
    let sep = if header.contains(';') && !header.contains(',') { ';' } else { ',' };
    let cols: Vec<String> = header.split(sep).map(|c| c.trim().trim_matches('"').to_lowercase()).collect();
    let find = |names: &[&str]| cols.iter().position(|c| names.contains(&c.as_str()));
    let t = find(&["time", "timestamp", "date", "datetime", "open_time", "ts"]).context("no time column")?;
    let o = find(&["open", "o"]).context("no open column")?;
    let h = find(&["high", "h"]).context("no high column")?;
    let l = find(&["low", "l"]).context("no low column")?;
    let c = find(&["close", "c", "adj close"]).context("no close column")?;
    let v = find(&["volume", "vol", "v"]);
    let mut out = Vec::new();
    for (n, line) in lines.enumerate() {
        let f: Vec<&str> = line.split(sep).map(|x| x.trim().trim_matches('"')).collect();
        let num = |i: usize, what: &str| -> anyhow::Result<f64> {
            f.get(i).and_then(|s| s.parse().ok()).with_context(|| format!("line {}: bad {what}", n + 2))
        };
        let ts = f.get(t).and_then(|s| time::parse(s)).with_context(|| format!("line {}: bad time", n + 2))?;
        let bar = Candle {
            ts,
            open: num(o, "open")?,
            high: num(h, "high")?,
            low: num(l, "low")?,
            close: num(c, "close")?,
            volume: v.map(|i| num(i, "volume")).transpose()?.unwrap_or(0.0),
        };
        if bar.high < bar.low {
            bail!("line {}: high is below low", n + 2);
        }
        out.push(bar);
    }
    out.sort_by_key(|c| c.ts);
    out.dedup_by_key(|c| c.ts);
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_common_layouts() {
        let a = parse("time,open,high,low,close,volume\n1700000000000,1,2,0.5,1.5,10\n").unwrap();
        assert_eq!(a[0].ts, 1_700_000_000_000);
        let b = parse("Date;Open;High;Low;Close\n2024-01-02;10;12;9;11\n2024-01-01;9;10;8;10\n").unwrap();
        assert_eq!(b.len(), 2);
        assert!(b[0].ts < b[1].ts, "sorted");
        assert_eq!(b[0].volume, 0.0);
        let c = parse("timestamp,open,high,low,close,volume\n1700000000,1,2,1,2,3\n").unwrap();
        assert_eq!(c[0].ts, 1_700_000_000_000, "seconds are converted");
        assert!(parse("date,price\n2024-01-01,1\n").is_err());
    }
}
