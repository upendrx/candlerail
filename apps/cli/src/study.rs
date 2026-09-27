//! Runs a parameter study: loads every market and timeframe once, then
//! backtests every variant on all of them in parallel.

use crate::now_ms;
use anyhow::Result;
use candlerail_core::study::{self, DataRange, Digest, MarketResult, Study, StudyResult, Summary};
use candlerail_core::{Candle, Interval, Strategy};
use candlerail_data::{Query, binance};
use std::collections::BTreeMap;
use std::path::Path;
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

/// A study can be written in the app's own format or passed as the path to one.
pub fn parse(text: &str) -> Result<Study> {
    serde_json::from_str(text).map_err(|e| anyhow::anyhow!("not a valid study file: {e}"))
}

/// Results by (family, variant, interval, cost), one per market.
type Results = BTreeMap<(usize, usize, Interval, usize), Vec<MarketResult>>;

struct Job {
    family: usize,
    variant: usize,
    interval: Interval,
    cost: usize,
    strategy: Strategy,
}

/// Loads the data and runs everything. `progress` gets (done, total) as
/// backtests finish, and messages while candles load.
pub fn run(study: &Study, api: &str, cache: &Path, progress: &(dyn Fn(Progress) + Sync)) -> Result<StudyResult> {
    let step_to = |i: Interval| (now_ms() / i.millis()) * i.millis();
    // Every timeframe ends at the same moment, the latest fully closed candle of the slowest one.
    let to = study.intervals.iter().map(|&i| step_to(i)).min().unwrap_or_else(now_ms)
        - study.offset_days as i64 * 86_400_000;
    let from = to - study.days as i64 * 86_400_000;
    let split_ts = from + ((to - from) as f64 * study.split) as i64;

    let mut data: BTreeMap<(String, Interval), Vec<Candle>> = BTreeMap::new();
    let mut ranges = vec![];
    for sym in &study.symbols {
        for &interval in &study.intervals {
            progress(Progress::Loading { symbol: sym.clone(), interval });
            let q = Query { symbol: sym.to_uppercase(), interval, from, to };
            let candles = binance::load(api, cache, &q, |_| {})?;
            ranges.push(DataRange {
                symbol: q.symbol.clone(),
                interval,
                from: candles.first().map_or(from, |c| c.ts),
                to: candles.last().map_or(to, |c| c.ts),
                bars: candles.len(),
            });
            data.insert((q.symbol, interval), candles);
        }
    }

    let mut jobs = vec![];
    let mut errors = vec![];
    let mut variants = vec![];
    for (fi, fam) in study.families.iter().enumerate() {
        let vs = study::variants(study, fam);
        for v in &vs {
            for &interval in &study.intervals {
                for (ci, cost) in study.costs.iter().enumerate() {
                    match study::instantiate(fam, v, interval, cost) {
                        Ok(strategy) => jobs.push(Job { family: fi, variant: v.index, interval, cost: ci, strategy }),
                        Err(e) => {
                            if !errors.contains(&e) {
                                errors.push(e)
                            }
                        }
                    }
                }
            }
        }
        variants.push(vs);
    }

    let symbols: Vec<String> = study.symbols.iter().map(|s| s.to_uppercase()).collect();
    let total = jobs.len() * symbols.len();
    let next = AtomicUsize::new(0);
    let done = AtomicUsize::new(0);
    let results: Mutex<Results> = Mutex::new(BTreeMap::new());
    let run_errors = Mutex::new(Vec::<String>::new());
    let threads = std::thread::available_parallelism().map_or(4, |n| n.get());
    std::thread::scope(|scope| {
        for _ in 0..threads {
            scope.spawn(|| {
                loop {
                    let k = next.fetch_add(1, Ordering::Relaxed);
                    if k >= total {
                        break;
                    }
                    let (job, sym) = (&jobs[k / symbols.len()], &symbols[k % symbols.len()]);
                    let candles = &data[&(sym.clone(), job.interval)];
                    match study::evaluate(&job.strategy, sym, candles, study.capital, job.interval, split_ts) {
                        Ok(m) => results
                            .lock()
                            .expect("results lock")
                            .entry((job.family, job.variant, job.interval, job.cost))
                            .or_default()
                            .push(m),
                        Err(e) => {
                            let msg = format!("{} on {sym}: {}", job.strategy.name, e.join("; "));
                            let mut errs = run_errors.lock().expect("errors lock");
                            if errs.len() < 50 {
                                errs.push(msg);
                            }
                        }
                    }
                    let d = done.fetch_add(1, Ordering::Relaxed) + 1;
                    if d.is_multiple_of(50) || d == total {
                        progress(Progress::Testing { done: d, total });
                    }
                }
            });
        }
    });

    let mut summaries: Vec<Summary> = vec![];
    for ((fi, vi, interval, ci), mut markets) in results.into_inner().expect("results lock") {
        markets.sort_by(|a, b| a.symbol.cmp(&b.symbol));
        summaries.push(study::summarize(
            &study.families[fi].id,
            &variants[fi][vi],
            interval,
            ci,
            markets,
            &study.selection,
        ));
    }
    errors.extend(run_errors.into_inner().expect("errors lock"));
    Ok(StudyResult { study: study.clone(), generated_at: now_ms(), split_ts, data: ranges, summaries, errors })
}

pub enum Progress {
    Loading { symbol: String, interval: Interval },
    Testing { done: usize, total: usize },
}

/// A short text report of the variants that were selected, best in sample first.
pub fn print(r: &StudyResult) {
    let s = &r.study;
    let sel: Vec<&Summary> = r.summaries.iter().filter(|x| x.selected).collect();
    let surv = sel.iter().filter(|x| x.survived).count();
    println!(
        "\n{}: {} variants × markets tested, {} selected in sample, {} of those stayed profitable out of sample.",
        s.name,
        r.summaries.len() * s.symbols.len(),
        sel.len(),
        surv
    );
    println!(
        "Selected on {} to {}; judged on {} to {}.\n",
        candlerail_core::time::format(r.data.iter().map(|d| d.from).min().unwrap_or(0)),
        candlerail_core::time::format(r.split_ts),
        candlerail_core::time::format(r.split_ts),
        candlerail_core::time::format(r.data.iter().map(|d| d.to).max().unwrap_or(0)),
    );
    let mut rows = sel;
    rows.sort_by(|a, b| b.in_sample.avg_r().partial_cmp(&a.in_sample.avg_r()).unwrap_or(std::cmp::Ordering::Equal));
    println!(
        "{:<24} {:>4} {:<10} {:>7} {:>8} {:>8} {:>6} {:>7}  settings",
        "family", "tf", "costs", "tr/day", "R in", "R out", "mkts", "fees R"
    );
    let r2 = |v: Option<f64>| v.map_or("-".into(), |x| format!("{x:+.3}"));
    for x in rows.iter().take(25) {
        let params: Vec<String> = x.params.iter().map(|(k, v)| format!("{k}={v}")).collect();
        println!(
            "{:<24} {:>4} {:<10} {:>7.1} {:>8} {:>8} {:>3}/{:<2} {:>7} {} {}",
            x.family,
            x.interval.as_str(),
            s.costs[x.cost].label.chars().take(10).collect::<String>(),
            x.trades_per_day,
            r2(x.in_sample.avg_r()),
            r2(x.out_of_sample.avg_r()),
            x.markets_up_out_of_sample,
            x.markets.len(),
            r2(x.fees_r),
            if x.survived { "✓" } else { " " },
            params.join(" ")
        );
    }
    if !r.errors.is_empty() {
        eprintln!("\n{} template problem(s); first: {}", r.errors.len(), r.errors[0]);
    }
}

/// The result as JSON, with numbers rounded to 5 significant digits so the
/// file stays small enough to ship with the app.
pub fn to_json<T: serde::Serialize>(r: &T) -> Result<String> {
    fn round(v: &mut serde_json::Value) {
        match v {
            serde_json::Value::Number(n) if n.is_f64() => {
                let x = n.as_f64().unwrap_or(0.0);
                if x != 0.0 && x.is_finite() {
                    let mag = 10f64.powi(4 - x.abs().log10().floor() as i32);
                    let y = (x * mag).round() / mag;
                    if let Some(num) = serde_json::Number::from_f64(y) {
                        *n = num;
                    }
                }
            }
            serde_json::Value::Array(a) => a.iter_mut().for_each(round),
            serde_json::Value::Object(m) => m.values_mut().for_each(round),
            _ => {}
        }
    }
    let mut v = serde_json::to_value(r)?;
    round(&mut v);
    Ok(serde_json::to_string(&v)?)
}

/// Recomputes one variant's full detail on the exact data window a study
/// recorded (from the cache, downloading what's missing).
pub fn detail(
    d: &Digest,
    family: usize,
    variant: usize,
    interval: Interval,
    cost: usize,
    api: &str,
    cache: &Path,
) -> Result<Summary> {
    let s = &d.study;
    let fam = s.families.get(family).ok_or_else(|| anyhow::anyhow!("no family {family}"))?;
    let v = study::variants(s, fam).into_iter().nth(variant).ok_or_else(|| anyhow::anyhow!("no variant {variant}"))?;
    let c = s.costs.get(cost).ok_or_else(|| anyhow::anyhow!("no cost scenario {cost}"))?;
    let strategy = study::instantiate(fam, &v, interval, c).map_err(anyhow::Error::msg)?;
    let mut markets = vec![];
    for r in d.data.iter().filter(|r| r.interval == interval) {
        let q = Query { symbol: r.symbol.clone(), interval, from: r.from, to: r.to + interval.millis() };
        let candles = binance::load(api, cache, &q, |_| {})?;
        markets.push(
            study::evaluate(&strategy, &r.symbol, &candles, s.capital, interval, d.split_ts)
                .map_err(|e| anyhow::anyhow!(e.join("; ")))?,
        );
    }
    Ok(study::summarize(&fam.id, &v, interval, cost, markets, &s.selection))
}
