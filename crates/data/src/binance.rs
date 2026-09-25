//! Binance spot klines through the public market-data API (no key).

use crate::{Query, cache, tidy};
use anyhow::{Context, bail};
use candlerail_core::Candle;
use std::path::Path;

pub const DEFAULT_API: &str = "https://data-api.binance.vision";

fn get(url: &str) -> anyhow::Result<serde_json::Value> {
    let mut last = None;
    for attempt in 0..3u64 {
        if attempt > 0 {
            std::thread::sleep(std::time::Duration::from_millis(400 * attempt));
        }
        match ureq::get(url)
            .header("User-Agent", concat!("candlerail/", env!("CARGO_PKG_VERSION")))
            .call()
            .and_then(|mut r| r.body_mut().read_to_string())
        {
            Ok(body) => return serde_json::from_str(&body).with_context(|| format!("parsing {url}")),
            Err(e) => last = Some(e),
        }
    }
    Err(last.expect("attempted")).with_context(|| format!("GET {url}"))
}

fn num(v: &serde_json::Value) -> Option<f64> {
    v.as_str()?.parse().ok()
}

/// Downloads candles straight from the API, 1,000 per request.
pub fn download(api: &str, q: &Query, mut progress: impl FnMut(usize)) -> anyhow::Result<Vec<Candle>> {
    let step = q.interval.millis();
    let mut out = Vec::new();
    let mut start = q.from;
    while start < q.to {
        let url = format!(
            "{}/api/v3/klines?symbol={}&interval={}&startTime={start}&endTime={}&limit=1000",
            api.trim_end_matches('/'),
            q.symbol.to_uppercase(),
            q.interval,
            q.to - 1
        );
        let v = get(&url)?;
        let Some(rows) = v.as_array() else {
            bail!("Binance returned an error for {}: {v}", q.symbol);
        };
        if rows.is_empty() {
            break;
        }
        for r in rows {
            let c = Candle {
                ts: r[0].as_i64().context("bad kline time")?,
                open: num(&r[1]).context("bad open")?,
                high: num(&r[2]).context("bad high")?,
                low: num(&r[3]).context("bad low")?,
                close: num(&r[4]).context("bad close")?,
                volume: num(&r[5]).context("bad volume")?,
            };
            start = c.ts + step;
            out.push(c);
        }
        progress(out.len());
        if rows.len() < 1000 {
            break;
        }
    }
    Ok(tidy(out, q.from, q.to))
}

/// Candles for the query, from the cache where possible. Only missing ranges
/// at either end are downloaded; bars that haven't closed yet are never cached.
pub fn load(api: &str, cache_dir: &Path, q: &Query, mut progress: impl FnMut(usize)) -> anyhow::Result<Vec<Candle>> {
    let file = cache::path(cache_dir, "binance", &q.symbol, q.interval);
    let mut have = cache::read(&file).unwrap_or_default();
    let step = q.interval.millis();
    let now =
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as i64).unwrap_or(0);
    let closed_until = (now / step) * step; // start of the bar that's still open
    let to = q.to.min(closed_until);
    let mut fetched = false;
    let first = have.first().map(|c| c.ts);
    let last = have.last().map(|c| c.ts);
    match (first, last) {
        (Some(f), Some(l)) => {
            if q.from < f {
                have.extend(download(api, &Query { to: f, ..q.clone() }, &mut progress)?);
                fetched = true;
            }
            if l + step < to {
                have.extend(download(api, &Query { from: l + step, to, ..q.clone() }, &mut progress)?);
                fetched = true;
            }
        }
        _ => {
            have = download(api, &Query { to, ..q.clone() }, &mut progress)?;
            fetched = true;
        }
    }
    let have = tidy(have, i64::MIN, closed_until);
    if fetched {
        cache::write(&file, &have)?;
    }
    Ok(tidy(have, q.from, q.to))
}
