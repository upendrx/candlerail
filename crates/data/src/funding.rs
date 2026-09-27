//! Funding-rate history for Binance USDⓈ-M perpetual futures, through the
//! public futures API (no key), cached on disk.

use anyhow::{Context, bail};
use candlerail_core::carry::FundingRate;
use std::path::{Path, PathBuf};

pub const DEFAULT_API: &str = "https://fapi.binance.com";

fn path(dir: &Path, symbol: &str) -> PathBuf {
    dir.join(format!("binance-funding-{}.csv", symbol.to_uppercase()))
}

fn read(file: &Path) -> Vec<FundingRate> {
    let Ok(text) = std::fs::read_to_string(file) else { return vec![] };
    text.lines()
        .skip(1)
        .filter_map(|l| {
            let (ts, rate) = l.split_once(',')?;
            Some(FundingRate { ts: ts.trim().parse().ok()?, rate: rate.trim().parse().ok()? })
        })
        .collect()
}

fn write(file: &Path, rates: &[FundingRate]) -> anyhow::Result<()> {
    if let Some(dir) = file.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let mut out = String::from("ts,rate\n");
    for r in rates {
        out.push_str(&format!("{},{}\n", r.ts, r.rate));
    }
    std::fs::write(file, out).with_context(|| format!("writing {}", file.display()))
}

/// Downloads funding payments in [from, to), 1,000 per request.
pub fn download(api: &str, symbol: &str, from: i64, to: i64) -> anyhow::Result<Vec<FundingRate>> {
    let mut out: Vec<FundingRate> = vec![];
    let mut start = from;
    while start < to {
        let url = format!(
            "{}/fapi/v1/fundingRate?symbol={}&startTime={start}&endTime={}&limit=1000",
            api.trim_end_matches('/'),
            symbol.to_uppercase(),
            to - 1
        );
        let v = crate::binance::get(&url)?;
        let Some(rows) = v.as_array() else {
            bail!("Binance returned an error for {symbol} funding: {v}");
        };
        if rows.is_empty() {
            break;
        }
        for r in rows {
            let ts = r["fundingTime"].as_i64().context("bad funding time")?;
            let rate: f64 = r["fundingRate"].as_str().and_then(|s| s.parse().ok()).context("bad funding rate")?;
            start = ts + 1;
            out.push(FundingRate { ts, rate });
        }
        if rows.len() < 1000 {
            break;
        }
    }
    Ok(out)
}

/// Funding payments in [from, to), from the cache where possible.
pub fn load(api: &str, cache_dir: &Path, symbol: &str, from: i64, to: i64) -> anyhow::Result<Vec<FundingRate>> {
    let file = path(cache_dir, symbol);
    let mut have = read(&file);
    let first = have.first().map(|r| r.ts);
    let last = have.last().map(|r| r.ts);
    let mut fetched = false;
    match (first, last) {
        (Some(f), Some(l)) => {
            // Funding is paid every 8 hours; a gap longer than that means data is missing.
            if from + 8 * 3_600_000 < f {
                have.extend(download(api, symbol, from, f)?);
                fetched = true;
            }
            if l + 8 * 3_600_000 < to {
                have.extend(download(api, symbol, l + 1, to)?);
                fetched = true;
            }
        }
        _ => {
            have = download(api, symbol, from, to)?;
            fetched = true;
        }
    }
    have.sort_by_key(|r| r.ts);
    have.dedup_by_key(|r| r.ts);
    if fetched {
        write(&file, &have)?;
    }
    have.retain(|r| r.ts >= from && r.ts < to);
    Ok(have)
}
