//! Share files: a strategy plus the result it produced, so others can see the
//! numbers and re-run the test to check them.
//!
//! ```json
//! { "candlerail_share": 1, "author": "...", "notes": "...",
//!   "strategy": { ...strategy file... },
//!   "result": { "symbol": "BTCUSDT", "interval": "4h", "from": "...", "return_pct": 12.3, ... } }
//! ```
//!
//! A share file loads anywhere a strategy file does.

use crate::backtest::Report;
use crate::candle::Interval;
use crate::spec::Strategy;
use crate::time;
use serde::{Deserialize, Serialize};

pub const FORMAT_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SharedResult {
    pub symbol: String,
    pub interval: String,
    pub from: String,
    pub to: String,
    pub bars: usize,
    pub trades: usize,
    pub return_pct: f64,
    pub buy_hold_pct: f64,
    pub max_drawdown_pct: f64,
    pub win_rate_pct: f64,
    pub profit_factor: Option<f64>,
    pub sharpe: f64,
    pub avg_r: Option<f64>,
    pub fees_paid: f64,
    pub starting_balance: f64,
    /// candlerail version that produced the result.
    pub version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Share {
    pub candlerail_share: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub author: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,
    pub strategy: Strategy,
    pub result: SharedResult,
}

impl Share {
    pub fn new(strategy: &Strategy, report: &Report, symbol: &str, interval: Interval, version: &str) -> Self {
        let m = &report.metrics;
        Share {
            candlerail_share: FORMAT_VERSION,
            author: None,
            notes: None,
            strategy: strategy.clone(),
            result: SharedResult {
                symbol: symbol.to_string(),
                interval: interval.to_string(),
                from: time::format(report.start_ts),
                to: time::format(report.end_ts),
                bars: report.bars,
                trades: m.trades,
                return_pct: round(m.total_return_pct),
                buy_hold_pct: round(m.buy_hold_return_pct),
                max_drawdown_pct: round(m.max_drawdown_pct),
                win_rate_pct: round(m.win_rate_pct),
                profit_factor: m.profit_factor.map(round),
                sharpe: round(m.sharpe),
                avg_r: m.avg_r.map(round),
                fees_paid: round(m.fees_paid),
                starting_balance: m.start_equity,
                version: version.to_string(),
            },
        }
    }
}

fn round(x: f64) -> f64 {
    (x * 100.0).round() / 100.0
}
