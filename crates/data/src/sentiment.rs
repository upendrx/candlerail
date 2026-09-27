//! The Crypto Fear & Greed index from alternative.me (daily since February
//! 2018, public, no key), cached on disk. 0 is extreme fear, 100 extreme greed.

use anyhow::Context;
use std::path::Path;

pub const DEFAULT_API: &str = "https://api.alternative.me";

/// (day start in ms, value 0–100), oldest first.
pub fn load(api: &str, cache_dir: &Path) -> anyhow::Result<Vec<(i64, f64)>> {
    let file = cache_dir.join("fear-greed.csv");
    let now =
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as i64).unwrap_or(0);
    if let Ok(text) = std::fs::read_to_string(&file) {
        let v: Vec<(i64, f64)> = text
            .lines()
            .skip(1)
            .filter_map(|l| {
                let (a, b) = l.split_once(',')?;
                Some((a.parse().ok()?, b.parse().ok()?))
            })
            .collect();
        if v.last().is_some_and(|(t, _)| now - t < 36 * 3_600_000) {
            return Ok(v);
        }
    }
    let url = format!("{}/fng/?limit=0&format=json", api.trim_end_matches('/'));
    let body = crate::binance::get(&url)?;
    let rows = body["data"].as_array().context("unexpected Fear & Greed response")?;
    let mut v: Vec<(i64, f64)> = rows
        .iter()
        .filter_map(|r| {
            let ts: i64 = r["timestamp"].as_str()?.parse().ok()?;
            let val: f64 = r["value"].as_str()?.parse().ok()?;
            Some((ts * 1000, val))
        })
        .collect();
    v.sort_by_key(|x| x.0);
    v.dedup_by_key(|x| x.0);
    std::fs::create_dir_all(cache_dir)?;
    let mut out = String::from("ts,value\n");
    for (t, x) in &v {
        out.push_str(&format!("{t},{x}\n"));
    }
    std::fs::write(&file, out)?;
    Ok(v)
}
