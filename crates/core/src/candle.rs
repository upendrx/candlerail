//! OHLCV candles and bar intervals.
//!
//! Prices are `f64`. At candle timescales (minutes to days) that's fine and
//! keeps strategy code simple; the tick-level engine (tickrail) uses integers.

use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;

#[derive(Copy, Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Candle {
    /// Open time, milliseconds since the Unix epoch (UTC).
    pub ts: i64,
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
    pub volume: f64,
}

impl Candle {
    /// Typical price, (high + low + close) / 3.
    pub fn hlc3(&self) -> f64 {
        (self.high + self.low + self.close) / 3.0
    }

    pub fn hl2(&self) -> f64 {
        (self.high + self.low) / 2.0
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Interval {
    #[serde(rename = "1m")]
    M1,
    #[serde(rename = "5m")]
    M5,
    #[serde(rename = "15m")]
    M15,
    #[serde(rename = "30m")]
    M30,
    #[serde(rename = "1h")]
    H1,
    #[serde(rename = "4h")]
    H4,
    #[serde(rename = "1d")]
    D1,
    #[serde(rename = "1w")]
    W1,
}

impl Interval {
    pub const ALL: [Interval; 8] = [
        Interval::M1,
        Interval::M5,
        Interval::M15,
        Interval::M30,
        Interval::H1,
        Interval::H4,
        Interval::D1,
        Interval::W1,
    ];

    pub fn millis(self) -> i64 {
        const M: i64 = 60_000;
        match self {
            Interval::M1 => M,
            Interval::M5 => 5 * M,
            Interval::M15 => 15 * M,
            Interval::M30 => 30 * M,
            Interval::H1 => 60 * M,
            Interval::H4 => 240 * M,
            Interval::D1 => 1_440 * M,
            Interval::W1 => 10_080 * M,
        }
    }

    /// Bars in a year, used to annualise per-bar statistics. Crypto trades
    /// around the clock, so this is calendar time.
    pub fn bars_per_year(self) -> f64 {
        365.0 * 86_400_000.0 / self.millis() as f64
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Interval::M1 => "1m",
            Interval::M5 => "5m",
            Interval::M15 => "15m",
            Interval::M30 => "30m",
            Interval::H1 => "1h",
            Interval::H4 => "4h",
            Interval::D1 => "1d",
            Interval::W1 => "1w",
        }
    }
}

impl fmt::Display for Interval {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for Interval {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, String> {
        Interval::ALL
            .into_iter()
            .find(|i| i.as_str() == s)
            .ok_or_else(|| format!("unknown interval `{s}` (use 1m, 5m, 15m, 30m, 1h, 4h, 1d or 1w)"))
    }
}
