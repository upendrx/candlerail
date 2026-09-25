//! Runs a strategy over candles and produces a [`Report`].
//!
//! For each bar, in order:
//!
//! 1. orders decided at the previous close fill at this bar's open;
//! 2. stops, targets and liquidation are checked against this bar's range;
//! 3. the bar closes: indicators update and rules are evaluated, which may
//!    queue an entry or exit for the next open;
//! 4. equity is marked at the close.

use crate::broker::{Broker, BrokerRules, Dist, EntryOrder, ExitReason, Side, SizeRule, TargetRule, Trade};
use crate::candle::{Candle, Interval};
use crate::metrics::{self, Metrics, Segment};
use crate::spec::{self, Compiled, History, SizingType, Strategy};
use serde::Serialize;

#[derive(Debug, Clone)]
pub struct BacktestConfig {
    pub capital: f64,
    pub interval: Interval,
    /// Fraction of bars treated as "in sample"; the rest checks for overfitting.
    pub split: f64,
}

impl Default for BacktestConfig {
    fn default() -> Self {
        BacktestConfig { capital: 10_000.0, interval: Interval::H1, split: 0.7 }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct Series {
    pub id: String,
    pub label: String,
    pub output: String,
    pub overlay: bool,
    /// One value per candle, `null` during warm-up.
    pub values: Vec<Option<f64>>,
}

#[derive(Debug, Clone, Serialize)]
pub struct EquityPoint {
    pub ts: i64,
    pub equity: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct Report {
    pub strategy: String,
    pub bars: usize,
    pub start_ts: i64,
    pub end_ts: i64,
    pub metrics: Metrics,
    pub in_sample: Segment,
    pub out_of_sample: Segment,
    pub trades: Vec<Trade>,
    pub equity: Vec<EquityPoint>,
    pub indicators: Vec<Series>,
    /// Plain-language cautions about how far to trust the result.
    pub warnings: Vec<String>,
}

fn rules_for(s: &Strategy) -> BrokerRules {
    let dist = |d: &spec::Distance| match (d.percent, d.atr) {
        (Some(p), _) => Dist::Percent(p),
        (_, Some(a)) => Dist::Atr(a),
        _ => Dist::Percent(0.0),
    };
    let x = &s.exit;
    BrokerRules {
        leverage: s.leverage,
        fee_rate: s.costs.fee_bps / 10_000.0,
        slippage_rate: s.costs.slippage_bps / 10_000.0,
        mmr: s.costs.maintenance_margin_percent / 100.0,
        size: match s.sizing.kind {
            SizingType::PercentEquity => SizeRule::PercentEquity(s.sizing.value),
            SizingType::RiskPercent => SizeRule::RiskPercent(s.sizing.value),
            SizingType::Fixed => SizeRule::Fixed(s.sizing.value),
        },
        stop: x.stop_loss.as_ref().map(dist),
        trail: x.trailing_stop.as_ref().map(dist),
        target: x.take_profit.as_ref().map(|t| match (t.percent, t.atr, t.risk_multiple) {
            (Some(p), _, _) => TargetRule::Percent(p),
            (_, Some(a), _) => TargetRule::Atr(a),
            (_, _, Some(m)) => TargetRule::RiskMultiple(m),
            _ => TargetRule::Percent(0.0),
        }),
    }
}

/// Runs a backtest. Fails only if the strategy doesn't validate or there are no candles.
pub fn run(strategy: &Strategy, candles: &[Candle], cfg: &BacktestConfig) -> Result<Report, Vec<String>> {
    if candles.is_empty() {
        return Err(vec!["no candles to test on".into()]);
    }
    let mut c: Compiled = spec::compile(strategy)?;
    let mut history = History::new(c.sources.len());
    let mut broker = Broker::new(cfg.capital, rules_for(strategy));
    let mut equity = Vec::with_capacity(candles.len());
    let mut series: Vec<Series> = c
        .slots
        .iter()
        .flat_map(|slot| {
            slot.info.outputs.iter().map(|o| Series {
                id: slot.id.clone(),
                label: slot.label.clone(),
                output: o.to_string(),
                overlay: slot.info.overlay && *o != "direction" && *o != "width",
                values: Vec::with_capacity(candles.len()),
            })
        })
        .collect();

    let mut pending_entry: Option<EntryOrder> = None;
    let mut pending_exit: Option<ExitReason> = None;
    let mut bars_in_market = 0usize;

    for bar in candles {
        // 1. Fill what was decided at the last close.
        if let Some(reason) = pending_exit.take() {
            broker.close_at_open(bar, reason);
        }
        if let Some(order) = pending_entry.take()
            && broker.position.is_none()
        {
            broker.open(order, bar);
        }
        // 2. Intrabar exits.
        if broker.position.is_some() {
            bars_in_market += 1;
            broker.check_bar(bar);
        }
        // 3. Close: update indicators and evaluate rules.
        c.push(bar, &mut history);
        let mut k = 0;
        for slot in &c.slots {
            for o in 0..slot.info.outputs.len() {
                series[k].values.push(slot.ind.get(o).filter(|v| v.is_finite()));
                k += 1;
            }
        }
        if let Some(p) = &mut broker.position {
            p.bars += 1;
            let rule = match p.side {
                Side::Long => c.exit_long.as_ref(),
                Side::Short => c.exit_short.as_ref(),
            };
            if rule.is_some_and(|r| spec::eval(r, &history)) {
                pending_exit = Some(ExitReason::ExitRule);
            } else if strategy.exit.max_bars.is_some_and(|m| p.bars >= m) {
                pending_exit = Some(ExitReason::MaxBars);
            }
        } else {
            let atr = |s: Option<usize>| s.and_then(|i| history.latest(i));
            let side = if c.entry_long.as_ref().is_some_and(|r| spec::eval(r, &history)) {
                Some(Side::Long)
            } else if c.entry_short.as_ref().is_some_and(|r| spec::eval(r, &history)) {
                Some(Side::Short)
            } else {
                None
            };
            if let Some(side) = side {
                let order = EntryOrder {
                    side,
                    stop_atr: atr(c.stop_atr),
                    trail_atr: atr(c.trail_atr),
                    target_atr: atr(c.target_atr),
                };
                // ATR-based exits need the ATR to be warmed up; skip the signal otherwise.
                let atr_ready = (c.stop_atr.is_none() || order.stop_atr.is_some())
                    && (c.trail_atr.is_none() || order.trail_atr.is_some())
                    && (c.target_atr.is_none() || order.target_atr.is_some());
                if atr_ready {
                    pending_entry = Some(order);
                }
            }
        }
        // 4. Mark to market.
        equity.push(EquityPoint { ts: bar.ts, equity: broker.equity(bar.close) });
    }
    let last = candles.last().expect("non-empty");
    if broker.position.is_some() {
        broker.close_at(last.close, last.ts, ExitReason::EndOfData);
        if let Some(e) = equity.last_mut() {
            e.equity = broker.cash;
        }
    }

    let m = metrics::compute(&broker.trades, &equity, candles, cfg, bars_in_market, broker.skipped);
    let split_idx = ((candles.len() as f64 * cfg.split.clamp(0.1, 0.95)) as usize).min(candles.len() - 1);
    let split_ts = candles[split_idx].ts;
    let in_sample = metrics::segment(&broker.trades, &equity[..=split_idx], |t| t.exit_ts < split_ts);
    let out_of_sample = metrics::segment(&broker.trades, &equity[split_idx..], |t| t.exit_ts >= split_ts);
    let warnings = metrics::warnings(strategy, &m, &in_sample, &out_of_sample, cfg);

    Ok(Report {
        strategy: strategy.name.clone(),
        bars: candles.len(),
        start_ts: candles[0].ts,
        end_ts: last.ts,
        metrics: m,
        in_sample,
        out_of_sample,
        trades: broker.trades,
        equity,
        indicators: series,
        warnings,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn candles(closes: &[f64]) -> Vec<Candle> {
        closes
            .iter()
            .enumerate()
            .map(|(i, &c)| Candle {
                ts: i as i64 * 3_600_000,
                open: c,
                high: c * 1.001,
                low: c * 0.999,
                close: c,
                volume: 1.0,
            })
            .collect()
    }

    fn strat(json: &str) -> Strategy {
        Strategy::from_json(json).unwrap()
    }

    #[test]
    fn signal_fills_at_next_open() {
        let s = strat(
            r#"{ "name": "t", "entry": { "long": { "left": "close", "op": ">", "right": 100.5 } },
                 "exit": { "max_bars": 1 }, "costs": { "fee_bps": 0, "slippage_bps": 0 } }"#,
        );
        let c = candles(&[100.0, 101.0, 105.0, 105.0]);
        let r = run(&s, &c, &BacktestConfig::default()).unwrap();
        // Signal at bar 1's close (101), filled at bar 2's open (105), not at 101.
        assert_eq!(r.trades[0].entry_price, 105.0);
        assert_eq!(r.trades[0].entry_ts, c[2].ts);
    }

    #[test]
    fn uptrend_long_makes_money_and_costs_reduce_it() {
        let closes: Vec<f64> = (0..200).map(|i| 100.0 + i as f64 * 0.5).collect();
        let base = r#"{ "name": "t", "indicators": { "f": { "type": "sma", "period": 5 }, "s": { "type": "sma", "period": 20 } },
            "entry": { "long": { "left": "f", "op": ">", "right": "s" } },
            "exit": { "long": { "left": "f", "op": "<", "right": "s" } }, COSTS }"#;
        let free = strat(&base.replace("COSTS", r#""costs": { "fee_bps": 0, "slippage_bps": 0 }"#));
        let paid = strat(&base.replace("COSTS", r#""costs": { "fee_bps": 50, "slippage_bps": 50 }"#));
        let a = run(&free, &candles(&closes), &BacktestConfig::default()).unwrap();
        let b = run(&paid, &candles(&closes), &BacktestConfig::default()).unwrap();
        assert!(a.metrics.total_return_pct > 0.0);
        assert!(b.metrics.total_return_pct < a.metrics.total_return_pct);
        assert_eq!(a.trades.last().unwrap().reason, ExitReason::EndOfData);
        assert_eq!(a.equity.len(), 200);
        assert!(a.warnings.iter().any(|w| w.contains("trades")), "too few trades warning");
    }

    #[test]
    fn invalid_strategies_are_rejected() {
        let s = strat(
            r#"{ "name": "t", "entry": { "long": { "left": "nope", "op": ">", "right": 1 } }, "exit": { "max_bars": 1 } }"#,
        );
        assert!(run(&s, &candles(&[1.0, 2.0]), &BacktestConfig::default()).is_err());
    }
}
