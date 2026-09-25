//! The pure part of candlerail: everything from candles to a backtest report,
//! with no network or file access. That makes it easy to test, and lets the
//! same code run in the CLI, the local web app, and later in live paper trading.
//!
//! ```text
//! candles ──▶ indicators ──▶ strategy rules ──▶ paper broker ──▶ report
//! ```

pub mod backtest;
pub mod broker;
pub mod candle;
pub mod explain;
pub mod indicators;
pub mod metrics;
pub mod share;
pub mod spec;
pub mod time;

pub use backtest::{BacktestConfig, Report, run};
pub use candle::{Candle, Interval};
pub use spec::Strategy;
