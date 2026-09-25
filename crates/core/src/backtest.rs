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
use crate::spec::{self, Compiled, RiskRules, SizingType, Strategy};
use crate::time;
use serde::Serialize;
use std::collections::BTreeMap;

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
    /// What the account-level money-management rules did.
    pub risk: RiskReport,
    /// Plain-language cautions about how far to trust the result.
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct RiskReport {
    /// When the max-drawdown rule stopped all trading, if it did.
    pub halted_at: Option<i64>,
    /// Entry signals the rules blocked, by rule.
    pub blocked_entries: BTreeMap<String, u32>,
    /// Positions the rules closed early.
    pub forced_exits: u32,
}

/// Applies [`RiskRules`] as the backtest runs.
struct Guard<'a> {
    r: &'a RiskRules,
    peak: f64,
    day: i64,
    day_start: f64,
    day_blocked: bool,
    month: i64,
    month_start: f64,
    month_blocked: bool,
    trades_today: u32,
    streak: u32,
    seen: usize,
    pause_left: u32,
    report: RiskReport,
}

impl<'a> Guard<'a> {
    fn new(r: &'a RiskRules, capital: f64, first_ts: i64) -> Self {
        Guard {
            r,
            peak: capital,
            day: time::day(first_ts),
            day_start: capital,
            day_blocked: false,
            month: time::month_index(first_ts),
            month_start: capital,
            month_blocked: false,
            trades_today: 0,
            streak: 0,
            seen: 0,
            pause_left: 0,
            report: RiskReport::default(),
        }
    }

    /// Start of a candle: roll the day and month using the last closing balance.
    fn new_bar(&mut self, ts: i64, last_equity: f64) {
        let (d, m) = (time::day(ts), time::month_index(ts));
        if d != self.day {
            self.day = d;
            self.day_start = last_equity;
            self.day_blocked = false;
            self.trades_today = 0;
        }
        if m != self.month {
            self.month = m;
            self.month_start = last_equity;
            self.month_blocked = false;
        }
        self.pause_left = self.pause_left.saturating_sub(1);
    }

    /// End of a candle. Returns true when open positions must be closed.
    fn on_close(&mut self, ts: i64, equity: f64, trades: &[Trade]) -> bool {
        for t in &trades[self.seen..] {
            self.streak = if t.pnl <= 0.0 { self.streak + 1 } else { 0 };
            if let Some(n) = self.r.pause_after_losses
                && self.streak >= n
            {
                self.pause_left = self.r.pause_bars.unwrap_or(24);
                self.streak = 0;
            }
        }
        self.seen = trades.len();
        self.peak = self.peak.max(equity);
        let lost = |from: f64| if from > 0.0 { 100.0 * (from - equity) / from } else { 0.0 };
        let mut close = false;
        if let Some(x) = self.r.max_drawdown_percent
            && self.report.halted_at.is_none()
            && lost(self.peak) >= x
        {
            self.report.halted_at = Some(ts);
            close = true;
        }
        if let Some(x) = self.r.daily_loss_percent
            && !self.day_blocked
            && lost(self.day_start) >= x
        {
            self.day_blocked = true;
            close = true;
        }
        if let Some(x) = self.r.monthly_loss_percent
            && !self.month_blocked
            && lost(self.month_start) >= x
        {
            self.month_blocked = true;
            close = true;
        }
        close
    }

    fn entry_allowed(&mut self) -> bool {
        let reason = if self.report.halted_at.is_some() {
            "max drawdown reached"
        } else if self.month_blocked {
            "monthly loss limit"
        } else if self.day_blocked {
            "daily loss limit"
        } else if self.r.max_trades_per_day.is_some_and(|m| self.trades_today >= m) {
            "max trades per day"
        } else if self.pause_left > 0 {
            "pause after losing streak"
        } else {
            return true;
        };
        *self.report.blocked_entries.entry(reason.to_string()).or_default() += 1;
        false
    }
}

fn rules_for(s: &Strategy) -> BrokerRules {
    let dist = |d: &spec::Distance| match (d.percent, d.atr) {
        (Some(p), _) => Dist::Percent(p),
        (_, Some(a)) => Dist::Atr(a),
        _ if d.is_level() => Dist::Level,
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
            _ if t.above.is_some() || t.below.is_some() => TargetRule::Level,
            _ => TargetRule::Percent(0.0),
        }),
    }
}

/// Candle patterns are on/off flags, not lines worth drawing.
fn charted(kind: &str) -> bool {
    kind != "patterns"
}

/// Runs a backtest. Fails only if the strategy doesn't validate or there are no candles.
pub fn run(strategy: &Strategy, candles: &[Candle], cfg: &BacktestConfig) -> Result<Report, Vec<String>> {
    if candles.is_empty() {
        return Err(vec!["no candles to test on".into()]);
    }
    let mut c: Compiled = spec::compile(strategy)?;
    let mut history = c.history();
    let mut broker = Broker::new(cfg.capital, rules_for(strategy));
    let mut equity = Vec::with_capacity(candles.len());
    let mut series: Vec<Series> = c
        .slots
        .iter()
        .filter(|slot| charted(slot.info.kind))
        .flat_map(|slot| {
            slot.info.outputs.iter().map(|o| Series {
                id: slot.id.clone(),
                label: slot.label.clone(),
                output: o.to_string(),
                overlay: slot.info.overlay && !["direction", "width", "structure", "ready"].contains(o),
                values: Vec::with_capacity(candles.len()),
            })
        })
        .collect();

    let mut pending_entry: Option<EntryOrder> = None;
    let mut pending_exit: Option<ExitReason> = None;
    let mut bars_in_market = 0usize;
    let mut guard = Guard::new(&strategy.risk, cfg.capital, candles[0].ts);

    for bar in candles {
        guard.new_bar(bar.ts, equity.last().map_or(cfg.capital, |e: &EquityPoint| e.equity));
        // 1. Fill what was decided at the last close.
        if let Some(reason) = pending_exit.take() {
            broker.close_at_open(bar, reason);
        }
        if let Some(order) = pending_entry.take()
            && broker.position.is_none()
        {
            broker.open(order, bar);
            if broker.position.is_some() {
                guard.trades_today += 1;
            }
        }
        // 2. Intrabar exits.
        if broker.position.is_some() {
            bars_in_market += 1;
            broker.check_bar(bar);
        }
        // 3. Close: update indicators and evaluate rules.
        c.push(bar, &mut history);
        let mut k = 0;
        for slot in c.slots.iter().filter(|slot| charted(slot.info.kind)) {
            for o in 0..slot.info.outputs.len() {
                series[k].values.push(slot.ind.get(o).filter(|v| v.is_finite()));
                k += 1;
            }
        }
        let must_close = guard.on_close(bar.ts, broker.equity(bar.close), &broker.trades);
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
            if must_close {
                pending_exit = Some(ExitReason::RiskLimit);
                guard.report.forced_exits += 1;
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
                let lvl = |v: Option<spec::Val>| v.and_then(|v| history.now(v));
                let (stop_level, target_level) = match side {
                    Side::Long => (lvl(c.stop_below), lvl(c.target_above)),
                    Side::Short => (lvl(c.stop_above), lvl(c.target_below)),
                };
                let levels_ready = (c.stop_below.is_none() && c.stop_above.is_none() || stop_level.is_some())
                    && (c.target_above.is_none() && c.target_below.is_none() || target_level.is_some());
                let order = EntryOrder {
                    side,
                    stop_level,
                    target_level,
                    stop_atr: atr(c.stop_atr),
                    trail_atr: atr(c.trail_atr),
                    target_atr: atr(c.target_atr),
                };
                // ATR-based exits need the ATR to be warmed up; skip the signal otherwise.
                let atr_ready = (c.stop_atr.is_none() || order.stop_atr.is_some())
                    && (c.trail_atr.is_none() || order.trail_atr.is_some())
                    && (c.target_atr.is_none() || order.target_atr.is_some());
                if atr_ready && levels_ready && guard.entry_allowed() {
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
    let mut warnings = metrics::warnings(strategy, &m, &in_sample, &out_of_sample, cfg);
    let risk = guard.report;
    if let Some(ts) = risk.halted_at {
        warnings.insert(
            0,
            format!(
                "The max-drawdown rule stopped all trading on {}. Results after that date are just the account sitting in cash.",
                time::format(ts)
            ),
        );
    }

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
        risk,
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
    fn money_management_rules_stop_trading() {
        // Price falls steadily: a long-only strategy that buys every bar loses on every trade.
        let closes: Vec<f64> = (0..300).map(|i| 1000.0 - i as f64 * 2.0).collect();
        let base = r#"{ "name": "t", "entry": { "long": { "left": "close", "op": ">", "right": 0 } },
            "exit": { "stop_loss": { "percent": 0.1 } }, "sizing": { "type": "risk_percent", "value": 2 },
            "costs": { "fee_bps": 0, "slippage_bps": 0 } RISK }"#;
        let free = run(&strat(&base.replace("RISK", "")), &candles(&closes), &BacktestConfig::default()).unwrap();
        let guarded = strat(&base.replace("RISK", r#", "risk": { "max_drawdown_percent": 10 }"#));
        let g = run(&guarded, &candles(&closes), &BacktestConfig::default()).unwrap();
        assert!(g.risk.halted_at.is_some());
        assert!(g.metrics.trades < free.metrics.trades);
        assert!(g.metrics.max_drawdown_pct < 12.0, "stopped near the limit: {}", g.metrics.max_drawdown_pct);
        assert!(g.risk.blocked_entries.get("max drawdown reached").copied().unwrap_or(0) > 0);
        let paused = strat(&base.replace("RISK", r#", "risk": { "pause_after_losses": 3, "pause_bars": 50 }"#));
        let p = run(&paused, &candles(&closes), &BacktestConfig::default()).unwrap();
        assert!(p.risk.blocked_entries.contains_key("pause after losing streak"));
        assert!(p.trades.iter().all(|t| t.r_multiple.is_some()), "every stopped trade has an R multiple");
    }

    #[test]
    fn invalid_strategies_are_rejected() {
        let s = strat(
            r#"{ "name": "t", "entry": { "long": { "left": "nope", "op": ">", "right": 1 } }, "exit": { "max_bars": 1 } }"#,
        );
        assert!(run(&s, &candles(&[1.0, 2.0]), &BacktestConfig::default()).is_err());
    }
}
