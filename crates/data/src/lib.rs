//! Where candles come from.
//!
//! * [`binance`]: public klines for any Binance spot pair, no account needed,
//!   cached on disk so repeated backtests don't hit the network.
//! * [`csv`]: any file with time, open, high, low, close and volume columns,
//!   which covers exports from most charting tools, brokers and data vendors.

pub mod binance;
pub mod cache;
pub mod csv;

use candlerail_core::{Candle, Interval};

/// A request for candles.
#[derive(Debug, Clone)]
pub struct Query {
    pub symbol: String,
    pub interval: Interval,
    /// Inclusive start, ms since epoch.
    pub from: i64,
    /// Exclusive end, ms since epoch.
    pub to: i64,
}

/// Keeps candles inside [from, to), sorted and without duplicates.
pub fn tidy(mut v: Vec<Candle>, from: i64, to: i64) -> Vec<Candle> {
    v.retain(|c| c.ts >= from && c.ts < to);
    v.sort_by_key(|c| c.ts);
    v.dedup_by_key(|c| c.ts);
    v
}
