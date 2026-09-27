//! Finds every candle where a strategy's entry conditions hold, without
//! trading. The chart lab uses it to show where a setup occurred, with exactly
//! the same rule evaluation a backtest uses.

use crate::candle::Candle;
use crate::spec::{self, Strategy};
use serde::Serialize;

/// Indices into the candle slice of the candles whose close satisfied each
/// side's entry condition.
#[derive(Debug, Clone, Default, Serialize, PartialEq)]
pub struct Matches {
    pub long: Vec<usize>,
    pub short: Vec<usize>,
}

/// Evaluates the entry rules at every close. Fails only if the strategy
/// doesn't compile.
pub fn scan(strategy: &Strategy, candles: &[Candle]) -> Result<Matches, Vec<String>> {
    scan_with_context(strategy, candles, &[])
}

/// [`scan`] with the candles of the strategy's context market.
pub fn scan_with_context(strategy: &Strategy, candles: &[Candle], context: &[Candle]) -> Result<Matches, Vec<String>> {
    let ctx = crate::backtest::align(candles, context);
    let mut c = spec::compile(strategy)?;
    let mut history = c.history();
    let mut out = Matches::default();
    for (i, bar) in candles.iter().enumerate() {
        c.push(bar, ctx[i], &mut history);
        if c.entry_long.as_ref().is_some_and(|r| spec::eval(r, &history)) {
            out.long.push(i);
        }
        if c.entry_short.as_ref().is_some_and(|r| spec::eval(r, &history)) {
            out.short.push(i);
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bar(i: i64, open: f64, high: f64, low: f64, close: f64) -> Candle {
        Candle { ts: i * 60_000, open, high, low, close, volume: 1.0 }
    }

    #[test]
    fn finds_every_matching_close() {
        let candles = vec![
            bar(0, 10.0, 11.0, 9.8, 10.5),
            bar(1, 10.5, 10.6, 8.0, 10.4), // long lower wick
            bar(2, 10.4, 12.0, 10.3, 11.9),
            bar(3, 11.9, 12.0, 9.5, 11.8), // long lower wick
        ];
        let s = Strategy::from_json(
            r#"{ "name": "wicks",
                 "entry": { "long": { "left": "lower_wick", "op": ">=", "right": "0.5 * range" },
                            "short": { "left": "close", "op": ">", "right": "high[1]" } },
                 "exit": { "max_bars": 2 } }"#,
        )
        .unwrap();
        let m = scan(&s, &candles).unwrap();
        assert_eq!(m.long, vec![1, 3]);
        assert_eq!(m.short, vec![2]);
    }

    #[test]
    fn reports_compile_errors() {
        let s = Strategy::from_json(
            r#"{ "name": "bad", "entry": { "long": { "left": "nope", "op": ">", "right": 1 } }, "exit": { "max_bars": 2 } }"#,
        )
        .unwrap();
        assert!(scan(&s, &[]).is_err());
    }

    #[test]
    fn rules_can_read_the_context_market() {
        let candles: Vec<Candle> = (0..6).map(|i| bar(i, 10.0, 11.0, 9.0, 10.5)).collect();
        // The context market is above 100 only on bars 2 and 3.
        let btc: Vec<Candle> = (0..6)
            .map(|i| {
                let p = if i == 2 || i == 3 { 105.0 } else { 95.0 };
                bar(i, p, p, p, p)
            })
            .collect();
        let s = Strategy::from_json(
            r#"{ "name": "btc up", "context": "BTCUSDT",
                 "indicators": { "btc": { "type": "sma", "period": 1, "on": "context" } },
                 "entry": { "long": { "left": "btc", "op": ">", "right": 100 } },
                 "exit": { "max_bars": 2 } }"#,
        )
        .unwrap();
        assert_eq!(scan_with_context(&s, &candles, &btc).unwrap().long, vec![2, 3]);
        assert!(scan(&s, &candles).unwrap().long.is_empty(), "without the context market, no value");
        let bad = Strategy::from_json(
            r#"{ "name": "x", "indicators": { "btc": { "type": "sma", "on": "context" } },
                 "entry": { "long": { "left": "btc", "op": ">", "right": 1 } }, "exit": { "max_bars": 2 } }"#,
        )
        .unwrap();
        assert!(bad.validate().iter().any(|e| e.contains("context market")));
    }
}
