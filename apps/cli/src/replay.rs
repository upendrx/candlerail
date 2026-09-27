//! Portfolio replay: backtest one strategy on many markets, then replay every
//! trade through a single shared account, the way a live bot would trade it.

use crate::now_ms;
use anyhow::Result;
use candlerail_core::quant::{self, Replay, ReplayTrade};
use candlerail_core::{BacktestConfig, Interval, Strategy, backtest, time};
use candlerail_data::{Query, binance};
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Run {
    pub risk_pct: f64,
    pub max_open: usize,
    pub all: Replay,
    pub before: Replay,
    pub after: Replay,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CostCase {
    pub label: String,
    pub fee_bps: f64,
    pub slippage_bps: f64,
    pub trades: usize,
    pub avg_r: f64,
    pub runs: Vec<Run>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Market {
    pub symbol: String,
    pub trades: usize,
    pub avg_r: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReplayResult {
    pub name: String,
    pub description: String,
    pub generated_at: i64,
    pub strategy: Strategy,
    pub interval: Interval,
    pub from: i64,
    pub to: i64,
    pub split_ts: i64,
    pub markets: Vec<Market>,
    pub costs: Vec<CostCase>,
}

#[allow(clippy::too_many_arguments)]
pub fn run(
    strategy: &Strategy,
    symbols: &[String],
    interval: Interval,
    from: i64,
    split: i64,
    risks: &[f64],
    max_opens: &[usize],
    costs: &[(String, f64, f64)],
    api: &str,
    cache: &Path,
) -> Result<ReplayResult> {
    let to = (now_ms() / interval.millis()) * interval.millis();
    let context = match &strategy.context {
        Some(c) => binance::load(api, cache, &Query { symbol: c.to_uppercase(), interval, from, to }, |_| {})?,
        None => vec![],
    };
    let mut data = vec![];
    for s in symbols {
        eprint!("\rloading {s} ...          ");
        let c = binance::load(api, cache, &Query { symbol: s.to_uppercase(), interval, from, to }, |_| {})?;
        if c.len() > 50 {
            data.push((s.to_uppercase(), c));
        }
    }
    eprintln!("\r                            ");
    let mut markets = vec![];
    let mut cases = vec![];
    for (ci, (label, fee, slip)) in costs.iter().enumerate() {
        let mut s = strategy.clone();
        s.costs.fee_bps = *fee;
        s.costs.slippage_bps = *slip;
        let mut trades = vec![];
        for (sym, candles) in &data {
            let cfg = BacktestConfig { interval, record_indicators: false, ..BacktestConfig::default() };
            let r =
                backtest::run_with_context(&s, candles, &context, &cfg).map_err(|e| anyhow::anyhow!(e.join("; ")))?;
            let mine: Vec<ReplayTrade> = r
                .trades
                .iter()
                .filter_map(|t| {
                    t.r_multiple.map(|r| ReplayTrade {
                        symbol: sym.clone(),
                        entry_ts: t.entry_ts,
                        exit_ts: t.exit_ts,
                        r,
                    })
                })
                .collect();
            if ci == 0 {
                let n = mine.len();
                markets.push(Market {
                    symbol: sym.clone(),
                    trades: n,
                    avg_r: if n > 0 { mine.iter().map(|t| t.r).sum::<f64>() / n as f64 } else { 0.0 },
                });
            }
            trades.extend(mine);
        }
        let mut runs = vec![];
        for &risk in risks {
            for &cap in max_opens {
                runs.push(Run {
                    risk_pct: risk,
                    max_open: cap,
                    all: quant::replay(&trades, risk, cap, from, to),
                    before: quant::replay(&trades, risk, cap, from, split),
                    after: quant::replay(&trades, risk, cap, split, to),
                });
            }
        }
        let n = trades.len();
        cases.push(CostCase {
            label: label.clone(),
            fee_bps: *fee,
            slippage_bps: *slip,
            trades: n,
            avg_r: if n > 0 { trades.iter().map(|t| t.r).sum::<f64>() / n as f64 } else { 0.0 },
            runs,
        });
    }
    Ok(ReplayResult {
        name: strategy.name.clone(),
        description: strategy.description.clone(),
        generated_at: now_ms(),
        strategy: strategy.clone(),
        interval,
        from,
        to,
        split_ts: split,
        markets,
        costs: cases,
    })
}

pub fn print(r: &ReplayResult) {
    println!("\n{}: {} markets, {} to {}", r.name, r.markets.len(), time::format(r.from), time::format(r.to));
    for c in &r.costs {
        println!("\n{} ({} trades, {:+.2}R average)", c.label, c.trades, c.avg_r);
        println!(
            "{:>6} {:>5} | {:>8} {:>8} {:>7} | {:>8} {:>8} | {:>7} {:>7} {:>6}",
            "risk", "open", "yr all", "fall", "mo up", "yr after", "fall", "taken", "skipped", "max"
        );
        for x in &c.runs {
            let up = x.all.monthly.iter().filter(|m| **m > 0.0).count();
            println!(
                "{:>5}% {:>5} | {:>7.1}% {:>7.1}% {:>3}/{:<3} | {:>7.1}% {:>7.1}% | {:>7} {:>7} {:>6}",
                x.risk_pct,
                x.max_open,
                x.all.annual_pct,
                x.all.max_drawdown_pct,
                up,
                x.all.monthly.len(),
                x.after.annual_pct,
                x.after.max_drawdown_pct,
                x.all.taken,
                x.all.skipped,
                x.all.max_open_seen
            );
        }
    }
}
