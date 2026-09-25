//! Paper broker for one instrument: market entries at the open, stop-loss,
//! take-profit, trailing stop, and liquidation under isolated margin.
//!
//! Fill rules are deliberately conservative, because an optimistic backtest
//! is worse than none:
//!
//! * signals are acted on at the **next bar's open**, never the bar that
//!   produced them (no look-ahead);
//! * inside a bar, price is assumed to hit the adverse extreme first, so when
//!   a bar touches both the stop and the target, the stop wins;
//! * a gap through a stop fills at the (worse) open;
//! * market and stop fills pay slippage, and every fill pays the fee;
//! * liquidation loses the whole position margin.

use crate::candle::Candle;
use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Side {
    Long,
    Short,
}

impl Side {
    pub fn sign(self) -> f64 {
        match self {
            Side::Long => 1.0,
            Side::Short => -1.0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ExitReason {
    StopLoss,
    TakeProfit,
    TrailingStop,
    Liquidation,
    ExitRule,
    MaxBars,
    /// Closed by an account-level money-management rule.
    RiskLimit,
    EndOfData,
}

#[derive(Debug, Clone, Serialize)]
pub struct Trade {
    pub side: Side,
    pub entry_ts: i64,
    pub exit_ts: i64,
    pub entry_price: f64,
    pub exit_price: f64,
    pub qty: f64,
    pub margin: f64,
    /// Net of fees.
    pub pnl: f64,
    /// PnL as a percentage of the margin put up.
    pub return_on_margin_pct: f64,
    pub fees: f64,
    pub bars: u32,
    pub reason: ExitReason,
    /// PnL in units of the initial risk (distance to the stop × size). `None` without a stop.
    pub r_multiple: Option<f64>,
}

#[derive(Debug, Clone)]
pub struct Position {
    pub side: Side,
    pub qty: f64,
    pub entry: f64,
    pub entry_ts: i64,
    pub margin: f64,
    pub stop: Option<f64>,
    pub target: Option<f64>,
    pub trail_dist: Option<f64>,
    pub trail: Option<f64>,
    pub liq: Option<f64>,
    pub best: f64,
    pub fees: f64,
    pub bars: u32,
    /// Money lost if the initial stop is hit, before costs.
    pub risk: Option<f64>,
}

impl Position {
    pub fn unrealized(&self, price: f64) -> f64 {
        self.side.sign() * (price - self.entry) * self.qty
    }
}

/// What the strategy asked for at the previous bar's close.
#[derive(Debug, Clone, Copy)]
pub struct EntryOrder {
    pub side: Side,
    /// Price-level stop and target, when the strategy uses levels.
    pub stop_level: Option<f64>,
    pub target_level: Option<f64>,
    /// Distances in price units, measured at signal time for ATR-based exits.
    pub stop_atr: Option<f64>,
    pub trail_atr: Option<f64>,
    pub target_atr: Option<f64>,
}

#[derive(Debug, Clone, Copy)]
pub enum SizeRule {
    PercentEquity(f64),
    RiskPercent(f64),
    Fixed(f64),
}

#[derive(Debug, Clone, Copy)]
pub enum Dist {
    Percent(f64),
    Atr(f64),
    /// At the level carried by the order.
    Level,
}

#[derive(Debug, Clone, Copy)]
pub enum TargetRule {
    Percent(f64),
    Atr(f64),
    RiskMultiple(f64),
    Level,
}

#[derive(Debug, Clone)]
pub struct BrokerRules {
    pub leverage: f64,
    pub fee_rate: f64,
    pub slippage_rate: f64,
    pub mmr: f64,
    pub size: SizeRule,
    pub stop: Option<Dist>,
    pub trail: Option<Dist>,
    pub target: Option<TargetRule>,
}

pub struct Broker {
    pub rules: BrokerRules,
    /// Cash balance: starting capital plus realised PnL minus fees.
    pub cash: f64,
    pub position: Option<Position>,
    pub trades: Vec<Trade>,
    /// Entries that couldn't be sized (e.g. fixed size larger than the account allows).
    pub skipped: u32,
}

impl Broker {
    pub fn new(capital: f64, rules: BrokerRules) -> Self {
        Broker { rules, cash: capital, position: None, trades: vec![], skipped: 0 }
    }

    pub fn equity(&self, mark: f64) -> f64 {
        self.cash + self.position.as_ref().map_or(0.0, |p| p.unrealized(mark))
    }

    fn slip(&self, price: f64, side: Side, buying: bool) -> f64 {
        let _ = side;
        if buying { price * (1.0 + self.rules.slippage_rate) } else { price * (1.0 - self.rules.slippage_rate) }
    }

    /// Opens a position at this bar's open.
    pub fn open(&mut self, o: EntryOrder, bar: &Candle) {
        let r = &self.rules;
        let buying = o.side == Side::Long;
        let fill = self.slip(bar.open, o.side, buying);
        let s = o.side.sign();
        let dist = |d: Dist, atr: Option<f64>| match d {
            Dist::Percent(p) => Some(fill * p / 100.0),
            Dist::Atr(m) => atr.map(|a| a * m),
            // Distance from the fill to the level, which must be on the losing side.
            Dist::Level => o.stop_level.map(|l| s * (fill - l)),
        };
        if matches!(r.stop, Some(Dist::Level)) && !dist(Dist::Level, None).is_some_and(|d| d > 0.0) {
            // The market opened through the planned stop: the setup is gone.
            self.skipped += 1;
            return;
        }
        let stop_dist = r.stop.and_then(|d| dist(d, o.stop_atr));
        let trail_dist = r.trail.and_then(|d| dist(d, o.trail_atr));
        let target_dist = match r.target {
            Some(TargetRule::Percent(p)) => Some(fill * p / 100.0),
            Some(TargetRule::Atr(m)) => o.target_atr.map(|a| a * m),
            Some(TargetRule::RiskMultiple(m)) => stop_dist.map(|d| d * m),
            Some(TargetRule::Level) => match o.target_level.map(|l| s * (l - fill)) {
                Some(d) if d > 0.0 => Some(d),
                // Already at or past the target at the open: nothing left to gain.
                _ => {
                    self.skipped += 1;
                    return;
                }
            },
            None => None,
        };
        let equity = self.cash;
        let max_notional = equity * r.leverage;
        let qty = match r.size {
            SizeRule::PercentEquity(pct) => equity * pct / 100.0 * r.leverage / fill,
            SizeRule::RiskPercent(pct) => match stop_dist {
                Some(d) if d > 0.0 => (equity * pct / 100.0 / d).min(max_notional / fill),
                _ => 0.0,
            },
            SizeRule::Fixed(q) => {
                if q * fill / r.leverage > equity {
                    0.0
                } else {
                    q
                }
            }
        };
        // Also catches NaN from a zero or missing stop distance.
        if qty.is_nan() || qty <= 0.0 || equity <= 0.0 {
            self.skipped += 1;
            return;
        }
        let notional = qty * fill;
        let margin = notional / r.leverage;
        let fee = notional * r.fee_rate;
        self.cash -= fee;
        let liq = (r.leverage > 1.0).then(|| fill * (1.0 - s / r.leverage + s * r.mmr));
        self.position = Some(Position {
            side: o.side,
            qty,
            entry: fill,
            entry_ts: bar.ts,
            margin,
            stop: stop_dist.map(|d| fill - s * d),
            target: target_dist.map(|d| fill + s * d),
            trail_dist,
            trail: trail_dist.map(|d| fill - s * d),
            liq,
            best: fill,
            fees: fee,
            bars: 0,
            risk: stop_dist.map(|d| d * qty).filter(|r| *r > 0.0),
        });
    }

    fn close(&mut self, price: f64, ts: i64, reason: ExitReason, pay_fee: bool) {
        let Some(p) = self.position.take() else { return };
        let fee = if pay_fee { p.qty * price * self.rules.fee_rate } else { 0.0 };
        let gross = if reason == ExitReason::Liquidation { -p.margin } else { p.unrealized(price) };
        self.cash += gross - fee;
        let pnl = gross - fee - p.fees;
        self.trades.push(Trade {
            side: p.side,
            entry_ts: p.entry_ts,
            exit_ts: ts,
            entry_price: p.entry,
            exit_price: price,
            qty: p.qty,
            margin: p.margin,
            pnl,
            return_on_margin_pct: if p.margin > 0.0 { 100.0 * pnl / p.margin } else { 0.0 },
            fees: p.fees + fee,
            bars: p.bars,
            reason,
            r_multiple: p.risk.map(|r| pnl / r),
        });
    }

    /// Closes at this bar's open (exit rules, max bars) with slippage.
    pub fn close_at_open(&mut self, bar: &Candle, reason: ExitReason) {
        let Some(p) = &self.position else { return };
        let price = self.slip(bar.open, p.side, p.side == Side::Short);
        self.close(price, bar.ts, reason, true);
    }

    /// Closes at a given price without slippage (end of data).
    pub fn close_at(&mut self, price: f64, ts: i64, reason: ExitReason) {
        self.close(price, ts, reason, true);
    }

    /// Checks stops, liquidation and target against this bar's range, then
    /// ratchets the trailing stop. Returns true if the position was closed.
    pub fn check_bar(&mut self, bar: &Candle) -> bool {
        let Some(p) = &self.position else { return false };
        let s = p.side.sign();
        let long = p.side == Side::Long;
        // Adverse levels: the tighter of the fixed and trailing stop, and liquidation.
        let (stop, stop_reason) = match (p.stop, p.trail) {
            (Some(fixed), Some(trail)) if (long && trail > fixed) || (!long && trail < fixed) => {
                (Some(trail), ExitReason::TrailingStop)
            }
            (Some(fixed), _) => (Some(fixed), ExitReason::StopLoss),
            (None, Some(trail)) => (Some(trail), ExitReason::TrailingStop),
            (None, None) => (None, ExitReason::StopLoss),
        };
        let adverse_extreme = if long { bar.low } else { bar.high };
        let beyond = |level: f64, price: f64| if long { price <= level } else { price >= level };
        // Whichever adverse level is closer to the open is hit first as price moves against us.
        let mut levels: Vec<(f64, ExitReason)> = vec![];
        if let Some(st) = stop {
            levels.push((st, stop_reason));
        }
        if let Some(l) = p.liq {
            levels.push((l, ExitReason::Liquidation));
        }
        levels.sort_by(|a, b| if long { b.0.total_cmp(&a.0) } else { a.0.total_cmp(&b.0) });
        for (level, reason) in levels {
            if beyond(level, adverse_extreme) {
                if reason == ExitReason::Liquidation {
                    self.close(level, bar.ts, reason, false);
                } else {
                    let raw = if beyond(level, bar.open) { bar.open } else { level };
                    let price = self.slip(raw, p.side, !long);
                    self.close(price, bar.ts, reason, true);
                }
                return true;
            }
        }
        if let Some(t) = p.target {
            let favourable = if long { bar.high } else { bar.low };
            let reached = if long { favourable >= t } else { favourable <= t };
            if reached {
                // A gap past the target fills at the (better) open; otherwise at the limit.
                let price = if (long && bar.open > t) || (!long && bar.open < t) { bar.open } else { t };
                self.close(price, bar.ts, ExitReason::TakeProfit, true);
                return true;
            }
        }
        if let Some(p) = &mut self.position {
            p.best = if long { p.best.max(bar.high) } else { p.best.min(bar.low) };
            if let (Some(d), Some(t)) = (p.trail_dist, p.trail) {
                let candidate = p.best - s * d;
                p.trail = Some(if long { t.max(candidate) } else { t.min(candidate) });
            }
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rules(lev: f64) -> BrokerRules {
        BrokerRules {
            leverage: lev,
            fee_rate: 0.0,
            slippage_rate: 0.0,
            mmr: 0.005,
            size: SizeRule::PercentEquity(100.0),
            stop: Some(Dist::Percent(2.0)),
            trail: None,
            target: Some(TargetRule::RiskMultiple(2.0)),
        }
    }

    fn bar(o: f64, h: f64, l: f64, c: f64) -> Candle {
        Candle { ts: 1, open: o, high: h, low: l, close: c, volume: 1.0 }
    }

    const LONG: EntryOrder = EntryOrder {
        side: Side::Long,
        stop_level: None,
        target_level: None,
        stop_atr: None,
        trail_atr: None,
        target_atr: None,
    };

    #[test]
    fn stop_beats_target_in_the_same_bar() {
        let mut b = Broker::new(1000.0, rules(1.0));
        b.open(LONG, &bar(100.0, 100.0, 100.0, 100.0));
        assert!(b.check_bar(&bar(100.0, 110.0, 90.0, 100.0)));
        assert_eq!(b.trades[0].reason, ExitReason::StopLoss);
        assert!((b.cash - 980.0).abs() < 1e-9, "lost 2% of 1000");
    }

    #[test]
    fn target_and_gap_fills() {
        let mut b = Broker::new(1000.0, rules(1.0));
        b.open(LONG, &bar(100.0, 100.0, 100.0, 100.0));
        assert!(b.check_bar(&bar(101.0, 104.5, 100.5, 104.0)));
        assert_eq!((b.trades[0].reason, b.trades[0].exit_price), (ExitReason::TakeProfit, 104.0));
        b.open(LONG, &bar(100.0, 100.0, 100.0, 100.0));
        assert!(b.check_bar(&bar(95.0, 96.0, 94.0, 95.0)), "gap below the stop");
        assert_eq!(b.trades[1].exit_price, 95.0, "fills at the worse open, not the stop");
    }

    #[test]
    fn leverage_and_liquidation() {
        let mut r = rules(10.0);
        r.stop = None;
        r.target = None;
        let mut b = Broker::new(1000.0, r);
        b.open(LONG, &bar(100.0, 100.0, 100.0, 100.0));
        let p = b.position.as_ref().unwrap();
        assert!((p.qty - 100.0).abs() < 1e-9, "100% margin at 10x = 10,000 notional");
        let liq = p.liq.unwrap();
        assert!((liq - 90.5).abs() < 1e-9, "10x long liquidates ~9.5% below entry");
        assert!(b.check_bar(&bar(99.0, 99.0, 90.0, 91.0)));
        assert_eq!(b.trades[0].reason, ExitReason::Liquidation);
        assert!((b.cash - 0.0).abs() < 1e-9, "whole margin lost");
    }

    #[test]
    fn trailing_stop_ratchets_up() {
        let mut r = rules(1.0);
        r.stop = None;
        r.target = None;
        r.trail = Some(Dist::Percent(5.0));
        let mut b = Broker::new(1000.0, r);
        b.open(LONG, &bar(100.0, 100.0, 100.0, 100.0));
        assert!(!b.check_bar(&bar(100.0, 120.0, 100.0, 119.0)));
        assert_eq!(b.position.as_ref().unwrap().trail, Some(115.0));
        assert!(b.check_bar(&bar(118.0, 118.0, 110.0, 111.0)));
        assert_eq!((b.trades[0].reason, b.trades[0].exit_price), (ExitReason::TrailingStop, 115.0));
    }

    #[test]
    fn level_stops_and_targets() {
        let mut r = rules(1.0);
        r.stop = Some(Dist::Level);
        r.target = Some(TargetRule::Level);
        r.size = SizeRule::RiskPercent(1.0);
        let mut b = Broker::new(10_000.0, r);
        let o = EntryOrder { stop_level: Some(95.0), target_level: Some(110.0), ..LONG };
        b.open(o, &bar(100.0, 100.0, 100.0, 100.0));
        let p = b.position.as_ref().unwrap();
        assert_eq!((p.stop, p.target), (Some(95.0), Some(110.0)));
        assert!((p.qty - 20.0).abs() < 1e-9, "1% of 10,000 over a 5-point stop");
        b.check_bar(&bar(100.0, 111.0, 99.0, 110.0));
        assert!((b.trades[0].r_multiple.unwrap() - 2.0).abs() < 1e-9, "a 10-point win on a 5-point risk is 2R");
        // Opening below the planned stop skips the trade.
        b.open(EntryOrder { stop_level: Some(95.0), ..o }, &bar(94.0, 94.0, 94.0, 94.0));
        assert!(b.position.is_none());
        assert_eq!(b.skipped, 1);
    }

    #[test]
    fn risk_sizing_loses_the_risked_amount() {
        let mut r = rules(5.0);
        r.size = SizeRule::RiskPercent(1.0);
        r.target = None;
        let mut b = Broker::new(10_000.0, r);
        b.open(LONG, &bar(100.0, 100.0, 100.0, 100.0));
        b.check_bar(&bar(100.0, 100.0, 97.0, 97.0));
        assert!((b.cash - 9_900.0).abs() < 1e-6, "1% of equity lost at the stop, got {}", b.cash);
    }
}
