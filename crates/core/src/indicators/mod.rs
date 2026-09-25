//! Streaming technical indicators.
//!
//! Every indicator is fed one closed candle at a time and exposes one or more
//! named outputs (MACD has `macd`, `signal` and `hist`, for example). During
//! warm-up an output is `None`, and rules that use it are simply false, so a
//! strategy never trades on half-computed values.
//!
//! To add an indicator: write a struct implementing [`Indicator`] in the
//! matching category file, add an entry to [`CATALOG`], and add an arm to
//! [`build`]. The catalog feeds the web builder, the AI prompt and the docs,
//! so a new indicator shows up everywhere at once.

mod momentum;
mod primitives;
mod trend;
mod volatility;
mod volume;

use crate::candle::Candle;
use serde::Serialize;
use serde_json::{Map, Value};

pub use primitives::{Ema, Rma, Sma};

pub trait Indicator: Send {
    /// Feeds one closed candle.
    fn update(&mut self, c: &Candle);
    /// Current value of output `index` (see [`IndicatorInfo::outputs`]).
    fn get(&self, index: usize) -> Option<f64>;
}

#[derive(Debug, Clone, Serialize)]
pub struct ParamInfo {
    pub name: &'static str,
    pub default: f64,
    pub min: f64,
    pub integer: bool,
    pub description: &'static str,
}

#[derive(Debug, Clone, Serialize)]
pub struct IndicatorInfo {
    /// Identifier used in strategy files, e.g. `"rsi"`.
    pub kind: &'static str,
    pub name: &'static str,
    pub category: &'static str,
    pub description: &'static str,
    pub params: &'static [ParamInfo],
    /// Output names; the first one is the default when a rule names just the indicator.
    pub outputs: &'static [&'static str],
    /// Drawn on the price chart (true) or in its own pane (false).
    pub overlay: bool,
}

const fn p(name: &'static str, default: f64, min: f64, integer: bool, description: &'static str) -> ParamInfo {
    ParamInfo { name, default, min, integer, description }
}

pub const CATALOG: &[IndicatorInfo] = &[
    IndicatorInfo {
        kind: "sma",
        name: "Simple moving average",
        category: "trend",
        description: "Average close over the last N bars. Smooths noise; price above a rising SMA is a basic uptrend.",
        params: &[p("period", 20.0, 1.0, true, "Number of bars")],
        outputs: &["value"],
        overlay: true,
    },
    IndicatorInfo {
        kind: "ema",
        name: "Exponential moving average",
        category: "trend",
        description: "Moving average that weights recent bars more, so it reacts faster than an SMA of the same length.",
        params: &[p("period", 20.0, 1.0, true, "Number of bars")],
        outputs: &["value"],
        overlay: true,
    },
    IndicatorInfo {
        kind: "rsi",
        name: "Relative strength index",
        category: "momentum",
        description: "0 to 100 gauge of recent gains versus losses (Wilder). Above 70 is often called overbought, below 30 oversold.",
        params: &[p("period", 14.0, 2.0, true, "Number of bars")],
        outputs: &["value"],
        overlay: false,
    },
    IndicatorInfo {
        kind: "macd",
        name: "MACD",
        category: "momentum",
        description: "Fast EMA minus slow EMA, with a signal line. The MACD crossing its signal line is a common momentum trigger.",
        params: &[
            p("fast", 12.0, 1.0, true, "Fast EMA length"),
            p("slow", 26.0, 2.0, true, "Slow EMA length"),
            p("signal", 9.0, 1.0, true, "Signal line EMA length"),
        ],
        outputs: &["macd", "signal", "hist"],
        overlay: false,
    },
    IndicatorInfo {
        kind: "bbands",
        name: "Bollinger Bands",
        category: "volatility",
        description: "SMA with bands a number of standard deviations above and below. Bands widen in volatile markets.",
        params: &[
            p("period", 20.0, 2.0, true, "Number of bars"),
            p("stddev", 2.0, 0.1, false, "Band width in standard deviations"),
        ],
        outputs: &["middle", "upper", "lower", "width"],
        overlay: true,
    },
    IndicatorInfo {
        kind: "atr",
        name: "Average true range",
        category: "volatility",
        description: "Average bar range including gaps (Wilder). Used to size stops to how much the market normally moves.",
        params: &[p("period", 14.0, 1.0, true, "Number of bars")],
        outputs: &["value"],
        overlay: false,
    },
    IndicatorInfo {
        kind: "stoch",
        name: "Stochastic oscillator",
        category: "momentum",
        description: "Where the close sits in the recent high-low range, 0 to 100. %K is the fast line, %D its average.",
        params: &[
            p("k_period", 14.0, 1.0, true, "Lookback for the high-low range"),
            p("k_smooth", 3.0, 1.0, true, "Smoothing of %K"),
            p("d_period", 3.0, 1.0, true, "Length of %D"),
        ],
        outputs: &["k", "d"],
        overlay: false,
    },
    IndicatorInfo {
        kind: "vwap",
        name: "VWAP (daily session)",
        category: "volume",
        description: "Volume-weighted average price since the start of the UTC day. Intraday traders use it as fair value.",
        params: &[],
        outputs: &["value"],
        overlay: true,
    },
    IndicatorInfo {
        kind: "supertrend",
        name: "SuperTrend",
        category: "trend",
        description: "ATR-based trailing line that flips sides with the trend. direction is 1 in an uptrend and -1 in a downtrend.",
        params: &[p("period", 10.0, 1.0, true, "ATR length"), p("multiplier", 3.0, 0.1, false, "ATR multiple")],
        outputs: &["value", "direction"],
        overlay: true,
    },
    IndicatorInfo {
        kind: "adx",
        name: "Average directional index",
        category: "trend",
        description: "Trend strength from 0 to 100, regardless of direction. Above 25 usually means a trending market.",
        params: &[p("period", 14.0, 2.0, true, "Number of bars")],
        outputs: &["adx", "plus_di", "minus_di"],
        overlay: false,
    },
    IndicatorInfo {
        kind: "obv",
        name: "On-balance volume",
        category: "volume",
        description: "Running total of volume, added on up closes and subtracted on down closes.",
        params: &[],
        outputs: &["value"],
        overlay: false,
    },
    IndicatorInfo {
        kind: "donchian",
        name: "Donchian channel",
        category: "volatility",
        description: "Highest high and lowest low of the last N bars, including this one. Compare with [1] to catch breakouts.",
        params: &[p("period", 20.0, 1.0, true, "Number of bars")],
        outputs: &["upper", "lower", "middle"],
        overlay: true,
    },
    IndicatorInfo {
        kind: "roc",
        name: "Rate of change",
        category: "momentum",
        description: "Percent change in close over N bars.",
        params: &[p("period", 10.0, 1.0, true, "Number of bars")],
        outputs: &["value"],
        overlay: false,
    },
    IndicatorInfo {
        kind: "keltner",
        name: "Keltner channel",
        category: "volatility",
        description: "EMA with bands a multiple of ATR above and below. Closes outside the channel signal strong moves.",
        params: &[
            p("period", 20.0, 1.0, true, "EMA length"),
            p("multiplier", 2.0, 0.1, false, "ATR multiple"),
            p("atr_period", 10.0, 1.0, true, "ATR length"),
        ],
        outputs: &["middle", "upper", "lower"],
        overlay: true,
    },
];

pub fn info(kind: &str) -> Option<&'static IndicatorInfo> {
    CATALOG.iter().find(|i| i.kind == kind)
}

/// Resolved parameter values, in catalog order, after defaults and validation.
pub fn resolve_params(info: &IndicatorInfo, given: &Map<String, Value>) -> Result<Vec<f64>, String> {
    for k in given.keys() {
        if k != "type" && !info.params.iter().any(|p| p.name == k) {
            let known: Vec<&str> = info.params.iter().map(|p| p.name).collect();
            return Err(format!("{} has no parameter `{k}` (it takes: {})", info.kind, known.join(", ")));
        }
    }
    info.params
        .iter()
        .map(|p| {
            let v = match given.get(p.name) {
                None => p.default,
                Some(v) => v.as_f64().ok_or_else(|| format!("{}.{} must be a number", info.kind, p.name))?,
            };
            if v < p.min {
                return Err(format!("{}.{} must be at least {}", info.kind, p.name, p.min));
            }
            if p.integer && v.fract() != 0.0 {
                return Err(format!("{}.{} must be a whole number", info.kind, p.name));
            }
            Ok(v)
        })
        .collect()
}

/// Short display label, e.g. `RSI(14)` or `BBANDS(20, 2)`.
pub fn label(info: &IndicatorInfo, params: &[f64]) -> String {
    if params.is_empty() {
        return info.kind.to_uppercase();
    }
    let ps: Vec<String> =
        params.iter().map(|v| if v.fract() == 0.0 { format!("{v:.0}") } else { format!("{v}") }).collect();
    format!("{}({})", info.kind.to_uppercase(), ps.join(", "))
}

pub fn build(kind: &str, params: &[f64]) -> Result<Box<dyn Indicator>, String> {
    let n = |i: usize| params[i] as usize;
    Ok(match kind {
        "sma" => Box::new(trend::SmaInd(Sma::new(n(0)))),
        "ema" => Box::new(trend::EmaInd(Ema::new(n(0)))),
        "rsi" => Box::new(momentum::Rsi::new(n(0))),
        "macd" => Box::new(momentum::Macd::new(n(0), n(1), n(2))),
        "bbands" => Box::new(volatility::Bbands::new(n(0), params[1])),
        "atr" => Box::new(volatility::Atr::new(n(0))),
        "stoch" => Box::new(momentum::Stoch::new(n(0), n(1), n(2))),
        "vwap" => Box::new(volume::Vwap::default()),
        "supertrend" => Box::new(trend::SuperTrend::new(n(0), params[1])),
        "adx" => Box::new(trend::Adx::new(n(0))),
        "obv" => Box::new(volume::Obv::default()),
        "donchian" => Box::new(volatility::Donchian::new(n(0))),
        "roc" => Box::new(momentum::Roc::new(n(0))),
        "keltner" => Box::new(volatility::Keltner::new(n(0), params[1], n(2))),
        other => return Err(format!("unknown indicator `{other}`")),
    })
}

#[cfg(test)]
mod tests;
