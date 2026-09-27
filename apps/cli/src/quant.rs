//! Portfolio studies: coin rotation, sentiment, seasonality and pairs, each
//! over a grid of settings, chosen on one period and judged on the next.

use crate::now_ms;
use anyhow::{Context, Result, bail};
use candlerail_core::quant::{self, Pair, Panel, Rotation, Run, Stats};
use candlerail_core::{Candle, Interval, time};
use candlerail_data::{Query, binance, sentiment};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Spec {
    pub id: String,
    pub name: String,
    pub description: String,
    /// rotation, sentiment, seasonality or pairs
    pub kind: String,
    pub symbols: Vec<String>,
    pub interval: Interval,
    pub from: String,
    /// First day of the held-out period.
    pub split: String,
    /// Cost per unit of weight traded, in basis points (fee plus slippage).
    pub cost_bps: f64,
    #[serde(default)]
    pub grid: BTreeMap<String, Vec<Value>>,
    /// Explicit combinations of settings, for rules whose settings differ.
    /// Used instead of `grid` when given.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub combos: Vec<BTreeMap<String, Value>>,
}

/// Fixed before any results are seen.
pub const SELECT_SHARPE: f64 = 1.0;
pub const SELECT_ANNUAL: f64 = 15.0;
pub const SURVIVE_SHARPE: f64 = 0.5;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Variant {
    pub params: BTreeMap<String, String>,
    pub in_sample: Stats,
    pub out_of_sample: Stats,
    pub monthly: Vec<f64>,
    pub trades_per_month: f64,
    pub turnover_per_year: f64,
    pub exposure: f64,
    pub selected: bool,
    pub survived: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuantResult {
    pub id: String,
    pub name: String,
    pub description: String,
    pub kind: String,
    pub generated_at: i64,
    pub from: i64,
    pub to: i64,
    pub split_ts: i64,
    pub month0: i64,
    pub selection: String,
    pub benchmark_label: String,
    pub benchmark_monthly: Vec<f64>,
    pub benchmark_in_sample: Stats,
    pub benchmark_out_of_sample: Stats,
    pub variants: Vec<Variant>,
    #[serde(default)]
    pub notes: Vec<String>,
}

fn combos(grid: &BTreeMap<String, Vec<Value>>) -> Vec<BTreeMap<String, Value>> {
    let mut out = vec![BTreeMap::new()];
    for (k, vals) in grid {
        out = out
            .into_iter()
            .flat_map(|m| {
                vals.iter().map(move |v| {
                    let mut m = m.clone();
                    m.insert(k.clone(), v.clone());
                    m
                })
            })
            .collect();
    }
    out
}

fn label(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Null => "off".into(),
        Value::Bool(b) => {
            if *b {
                "yes".into()
            } else {
                "no".into()
            }
        }
        other => other.to_string(),
    }
}

fn num(m: &BTreeMap<String, Value>, k: &str) -> Result<f64> {
    m.get(k).and_then(Value::as_f64).with_context(|| format!("grid setting `{k}` must be a number"))
}

fn opt(m: &BTreeMap<String, Value>, k: &str) -> Option<f64> {
    m.get(k).and_then(Value::as_f64)
}

fn load(
    symbols: &[String],
    interval: Interval,
    from: i64,
    to: i64,
    api: &str,
    cache: &Path,
) -> Result<Vec<(String, Vec<Candle>)>> {
    let mut out = vec![];
    for s in symbols {
        eprint!("\rloading {s} {interval} ...          ");
        let q = Query { symbol: s.to_uppercase(), interval, from, to };
        let c = binance::load(api, cache, &q, |_| {})?;
        if !c.is_empty() {
            out.push((q.symbol, c));
        }
    }
    eprintln!("\r                                          ");
    Ok(out)
}

pub fn run(spec: &Spec, api: &str, fng_api: &str, cache: &Path) -> Result<QuantResult> {
    let from = time::parse(&spec.from).context("bad `from` date")?;
    let split = time::parse(&spec.split).context("bad `split` date")?;
    let to = (now_ms() / 86_400_000) * 86_400_000;
    let series = load(&spec.symbols, spec.interval, from, to, api, cache)?;
    let p = Panel::new(&series);
    let r = p.returns();
    let bars_per_year = 365.0 * 86_400_000.0 / spec.interval.millis() as f64;
    let months_total = time::month_index(to) - time::month_index(from) + 1;
    let mut notes = vec![];

    // Buying and holding BTC, the yardstick.
    let btc = p.index("BTCUSDT").unwrap_or(0);
    let mut hold = vec![vec![0.0; p.ts.len()]; p.symbols.len()];
    for (h, c) in hold[btc].iter_mut().zip(&p.close[btc]) {
        if c.is_finite() {
            *h = 1.0;
        }
    }
    let bench = quant::apply(&r, &p.ts, &hold, 0.0);

    let fng: BTreeMap<i64, f64> =
        if spec.kind == "sentiment" { sentiment::load(fng_api, cache)?.into_iter().collect() } else { BTreeMap::new() };

    let mut variants = vec![];
    let grid = if spec.combos.is_empty() { combos(&spec.grid) } else { spec.combos.clone() };
    for (i, g) in grid.iter().enumerate() {
        if i % 20 == 0 {
            eprint!("\rtesting {}/{} ...     ", i + 1, grid.len());
        }
        let w = match spec.kind.as_str() {
            "rotation" => {
                let cfg = Rotation {
                    lookback: num(g, "lookback")? as usize,
                    every: num(g, "every")? as usize,
                    top: num(g, "top")? as usize,
                    long_short: g.get("long_short").and_then(Value::as_bool).unwrap_or(false),
                    btc_filter: opt(g, "btc_filter").map(|x| x as usize),
                    min_history: opt(g, "min_history").unwrap_or(60.0) as usize,
                    reverse: g.get("reverse").and_then(Value::as_bool).unwrap_or(false),
                };
                quant::rotation_weights(&p, &cfg)
            }
            "sentiment" => sentiment_weights(&p, &fng, g)?,
            "seasonality" => seasonal_weights(&p, &r, g, from, split)?,
            "pairs" => pairs_weights(&p, &r, g, from, split, &mut notes)?,
            other => bail!("unknown study kind `{other}`"),
        };
        let cost = match (spec.kind.as_str(), g.get("long_short").and_then(Value::as_bool)) {
            // Long-short needs futures, which cost less per trade.
            ("rotation", Some(true)) => opt(g, "futures_cost_bps").unwrap_or(spec.cost_bps / 2.0),
            _ => spec.cost_bps,
        };
        let run = quant::apply(&r, &p.ts, &w, cost);
        variants.push(variant(g, &run, from, split, to, bars_per_year, months_total));
    }
    eprintln!("\r                            ");
    Ok(QuantResult {
        id: spec.id.clone(),
        name: spec.name.clone(),
        description: spec.description.clone(),
        kind: spec.kind.clone(),
        generated_at: now_ms(),
        from,
        to,
        split_ts: split,
        month0: time::month_index(from),
        selection: format!(
            "Selected: Sharpe of at least {SELECT_SHARPE} and at least {SELECT_ANNUAL}% a year before {}. Survived: Sharpe of at least {SURVIVE_SHARPE} and a profit from then on.",
            spec.split
        ),
        benchmark_label: "Buy and hold BTC".into(),
        benchmark_monthly: quant::monthly(&bench, from, to).1,
        benchmark_in_sample: quant::stats(&bench, from, split, bars_per_year),
        benchmark_out_of_sample: quant::stats(&bench, split, to, bars_per_year),
        variants,
        notes,
    })
}

fn variant(g: &BTreeMap<String, Value>, run: &Run, from: i64, split: i64, to: i64, bpy: f64, months: i64) -> Variant {
    let is = quant::stats(run, from, split, bpy);
    let oos = quant::stats(run, split, to, bpy);
    let selected = is.sharpe >= SELECT_SHARPE && is.annual_pct >= SELECT_ANNUAL;
    let survived = selected && oos.sharpe >= SURVIVE_SHARPE && oos.annual_pct > 0.0;
    let years = (to - from) as f64 / (365.0 * 86_400_000.0);
    Variant {
        params: g.iter().map(|(k, v)| (k.clone(), label(v))).collect(),
        in_sample: is,
        out_of_sample: oos,
        monthly: quant::monthly(run, from, to).1,
        trades_per_month: run.trades as f64 / months.max(1) as f64,
        turnover_per_year: run.turnover / years.max(1e-9),
        exposure: run.exposure,
        selected,
        survived,
    }
}

/// Positions on one market (or an equal split of several) from the Fear & Greed index.
fn sentiment_weights(p: &Panel, fng: &BTreeMap<i64, f64>, g: &BTreeMap<String, Value>) -> Result<Vec<Vec<f64>>> {
    let rule = g.get("rule").and_then(Value::as_str).context("sentiment needs a `rule`")?;
    let assets: Vec<usize> = match g.get("asset").and_then(Value::as_str).unwrap_or("BTCUSDT") {
        "both" => ["BTCUSDT", "ETHUSDT"].iter().filter_map(|s| p.index(s)).collect(),
        s => p.index(s).into_iter().collect(),
    };
    let nt = p.ts.len();
    // The index for a day is published at its start; at that day's close we know it.
    let f: Vec<f64> =
        p.ts.iter().map(|t| fng.get(&((t / 86_400_000) * 86_400_000)).copied().unwrap_or(f64::NAN)).collect();
    let mut w = vec![vec![0.0; nt]; p.symbols.len()];
    let size = 1.0 / assets.len().max(1) as f64;
    for &a in &assets {
        let c = &p.close[a];
        let ma = |n: usize, t: usize| -> f64 {
            if t + 1 < n {
                return f64::NAN;
            }
            c[t + 1 - n..=t].iter().sum::<f64>() / n as f64
        };
        let pos = match rule {
            "buy_fear" => {
                let (lo, hi) = (num(g, "fear")?, num(g, "exit")?);
                quant::stateful(nt, size, |t| f[t] <= lo, |t| f[t] >= hi)
            }
            "trend_no_euphoria" => {
                let (n, cap) = (num(g, "ma")? as usize, num(g, "greed_cap")?);
                // Out when the trend is down or not yet known.
                let up = |t: usize| c[t] > ma(n, t);
                quant::stateful(nt, size, |t| up(t) && f[t] < cap, |t| !up(t) || f[t] >= cap)
            }
            "trend_buy_fear" => {
                let (n, lo) = (num(g, "ma")? as usize, num(g, "fear")?);
                // Buy fear only while the long-term trend is up; leave when the trend breaks.
                let up = |t: usize| c[t] > ma(n, t);
                quant::stateful(nt, size, |t| up(t) && f[t] <= lo, |t| !up(t))
            }
            "short_greed" => {
                let (n, hi) = (num(g, "ma")? as usize, num(g, "greed")?);
                quant::stateful(nt, -size, |t| c[t] < ma(n, t) && f[t] >= hi, |t| c[t] > ma(n, t))
            }
            other => bail!("unknown sentiment rule `{other}`"),
        };
        for t in 0..nt {
            w[a][t] = if c[t].is_finite() { pos[t] } else { 0.0 };
        }
    }
    Ok(w)
}

/// Holds during the hours (or weekdays) that did best in the selection period.
fn seasonal_weights(
    p: &Panel,
    r: &[Vec<f64>],
    g: &BTreeMap<String, Value>,
    from: i64,
    split: i64,
) -> Result<Vec<Vec<f64>>> {
    let weekday = g.get("by").and_then(Value::as_str) == Some("weekday");
    let k = num(g, "best")? as usize;
    let shorts = g.get("short_worst").and_then(Value::as_bool).unwrap_or(false);
    let assets: Vec<usize> = match g.get("asset").and_then(Value::as_str).unwrap_or("BTCUSDT") {
        "both" => ["BTCUSDT", "ETHUSDT"].iter().filter_map(|s| p.index(s)).collect(),
        s => p.index(s).into_iter().collect(),
    };
    let nt = p.ts.len();
    let mut w = vec![vec![0.0; nt]; p.symbols.len()];
    let size = 1.0 / assets.len().max(1) as f64;
    for &a in &assets {
        let means = quant::seasonal_means(&p.ts, &r[a], from, split, weekday);
        let mut order: Vec<usize> = (0..means.len()).collect();
        order.sort_by(|x, y| means[*y].0.partial_cmp(&means[*x].0).unwrap_or(std::cmp::Ordering::Equal));
        let best: Vec<usize> = order.iter().take(k).copied().filter(|s| means[*s].0 > 0.0).collect();
        let worst: Vec<usize> = order.iter().rev().take(k).copied().filter(|s| means[*s].0 < 0.0).collect();
        // Decide at each bar's close for the bar that starts next.
        for (x, next_ts) in w[a].iter_mut().zip(p.ts.iter().skip(1)) {
            let next = quant::slot(*next_ts, weekday);
            *x = if best.contains(&next) {
                size
            } else if shorts && worst.contains(&next) {
                -size
            } else {
                0.0
            };
        }
    }
    Ok(w)
}

/// Every pair of markets whose daily returns were strongly correlated in the
/// selection period, traded side by side with equal capital.
fn pairs_weights(
    p: &Panel,
    r: &[Vec<f64>],
    g: &BTreeMap<String, Value>,
    from: i64,
    split: i64,
    notes: &mut Vec<String>,
) -> Result<Vec<Vec<f64>>> {
    let cfg = Pair {
        window: num(g, "window")? as usize,
        entry: num(g, "entry")?,
        exit: num(g, "exit")?,
        max_hold: num(g, "max_hold")? as usize,
    };
    let min_corr = opt(g, "min_corr").unwrap_or(0.8);
    let a0 = p.ts.partition_point(|t| *t < from);
    let a1 = p.ts.partition_point(|t| *t < split);
    let n = p.symbols.len();
    let mut pairs = vec![];
    for a in 0..n {
        for b in a + 1..n {
            if quant::correlation(&r[a], &r[b], a0, a1).is_some_and(|c| c >= min_corr) {
                pairs.push((a, b));
            }
        }
    }
    let list: Vec<String> =
        pairs.iter().map(|(a, b)| format!("{}/{}", short(&p.symbols[*a]), short(&p.symbols[*b]))).collect();
    let note = format!(
        "Pairs with a correlation of at least {min_corr} before the split: {}",
        if list.is_empty() { "none".to_string() } else { list.join(", ") }
    );
    if !notes.contains(&note) {
        notes.push(note);
    }
    let mut w = vec![vec![0.0; p.ts.len()]; n];
    let size = 0.5 / pairs.len().max(1) as f64;
    for (a, b) in pairs {
        let (wa, wb) = quant::pair_weights(p, a, b, &cfg, size);
        for t in 0..p.ts.len() {
            w[a][t] += wa[t];
            w[b][t] += wb[t];
        }
    }
    Ok(w)
}

fn short(s: &str) -> &str {
    s.strip_suffix("USDT").unwrap_or(s)
}

pub fn print(q: &QuantResult) {
    let sel: Vec<&Variant> = q.variants.iter().filter(|v| v.selected).collect();
    println!(
        "\n{}: {} variants, {} selected, {} survived. Buy and hold BTC: {:+.1}% a year before the split, {:+.1}% after.",
        q.name,
        q.variants.len(),
        sel.len(),
        sel.iter().filter(|v| v.survived).count(),
        q.benchmark_in_sample.annual_pct,
        q.benchmark_out_of_sample.annual_pct
    );
    let mut rows: Vec<&Variant> = q.variants.iter().collect();
    rows.sort_by(|a, b| b.in_sample.sharpe.partial_cmp(&a.in_sample.sharpe).unwrap_or(std::cmp::Ordering::Equal));
    println!(
        "{:>7} {:>8} {:>8} | {:>7} {:>8} {:>8} {:>6} | settings",
        "Sh in", "yr in", "DD in", "Sh out", "yr out", "DD out", "tr/mo"
    );
    for v in rows.iter().take(15) {
        println!(
            "{:>7.2} {:>7.1}% {:>7.1}% | {:>7.2} {:>7.1}% {:>7.1}% {:>6.1} | {} {}",
            v.in_sample.sharpe,
            v.in_sample.annual_pct,
            v.in_sample.max_drawdown_pct,
            v.out_of_sample.sharpe,
            v.out_of_sample.annual_pct,
            v.out_of_sample.max_drawdown_pct,
            v.trades_per_month,
            if v.survived {
                "✓"
            } else if v.selected {
                "·"
            } else {
                " "
            },
            v.params.iter().map(|(k, x)| format!("{k}={x}")).collect::<Vec<_>>().join(" ")
        );
    }
    for n in &q.notes {
        println!("{n}");
    }
}
