//! The pure part of candlerail: everything from candles to a backtest report,
//! with no network or file access. That makes it easy to test, and lets the
//! same code run in the CLI, the local web app, and later in live paper trading.
//!
//! ```text
//! candles ──▶ indicators ──▶ strategy rules ──▶ paper broker ──▶ report
//! ```
pub mod candle;
pub mod indicators;
pub mod time;

pub use candle::{Candle, Interval};
