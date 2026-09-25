//! On-disk candle cache: one CSV per venue, symbol and interval.

use anyhow::Context;
use candlerail_core::{Candle, Interval};
use std::io::Write;
use std::path::{Path, PathBuf};

pub fn path(dir: &Path, venue: &str, symbol: &str, interval: Interval) -> PathBuf {
    dir.join(format!("{venue}-{}-{interval}.csv", symbol.to_uppercase()))
}

pub fn read(file: &Path) -> anyhow::Result<Vec<Candle>> {
    crate::csv::load(file)
}

pub fn write(file: &Path, candles: &[Candle]) -> anyhow::Result<()> {
    if let Some(d) = file.parent() {
        std::fs::create_dir_all(d).with_context(|| format!("creating {}", d.display()))?;
    }
    let tmp = file.with_extension("csv.tmp");
    let mut w = std::io::BufWriter::new(std::fs::File::create(&tmp)?);
    writeln!(w, "time,open,high,low,close,volume")?;
    for c in candles {
        writeln!(w, "{},{},{},{},{},{}", c.ts, c.open, c.high, c.low, c.close, c.volume)?;
    }
    w.flush()?;
    drop(w);
    std::fs::rename(&tmp, file)?;
    Ok(())
}
