//! The funding-rate carry trade: hold a coin on spot and short the same amount
//! of its perpetual future, so price moves cancel out and what's left is the
//! funding the short side receives (or pays) every eight hours.
//!
//! The account is split between the two legs: `hedge_fraction` of it buys the
//! coin, the same amount is posted as 1x margin for the short. Entry and exit
//! pay spot and futures fees plus slippage on both legs.

use crate::study::Months;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// One funding payment: the rate charged to longs and paid to shorts.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct FundingRate {
    pub ts: i64,
    pub rate: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CarryConfig {
    pub spot_fee_bps: f64,
    pub perp_fee_bps: f64,
    pub slippage_bps: f64,
    /// Share of the account in each leg (0.5 = half in spot, half as margin).
    pub hedge_fraction: f64,
}

impl Default for CarryConfig {
    fn default() -> Self {
        CarryConfig { spot_fee_bps: 10.0, perp_fee_bps: 5.0, slippage_bps: 2.0, hedge_fraction: 0.5 }
    }
}

impl CarryConfig {
    /// Cost of opening or closing both legs, as a % of the account.
    pub fn switch_cost_pct(&self) -> f64 {
        self.hedge_fraction * (self.spot_fee_bps + self.perp_fee_bps + 2.0 * self.slippage_bps) / 100.0
    }
}

/// When the hedge is held.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Mode {
    /// Open at the start, close at the end.
    Always,
    /// Hold only while the average funding of the last `days` days is positive.
    PositiveTrailing { days: u32 },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CarryResult {
    /// Profit month by month, as a % of the account, from the months' first.
    pub monthly: Vec<f64>,
    pub total_pct: f64,
    /// Times the hedge was opened or closed.
    pub switches: usize,
    pub costs_pct: f64,
    /// Share of funding payments that were negative (the short paid).
    pub negative_share: f64,
    /// Share of the time the hedge was held.
    pub held_share: f64,
}

pub fn run(rates: &[FundingRate], cfg: &CarryConfig, mode: Mode, months: Months) -> CarryResult {
    let mut monthly: BTreeMap<i64, f64> = BTreeMap::new();
    let cost = cfg.switch_cost_pct();
    let (mut held, mut switches, mut costs, mut held_n) = (false, 0usize, 0.0, 0usize);
    let window = match mode {
        Mode::Always => 0,
        Mode::PositiveTrailing { days } => (days as usize * 3).max(1),
    };
    let mut add = |ts: i64, v: f64| *monthly.entry(crate::time::month_index(ts)).or_insert(0.0) += v;
    for (i, r) in rates.iter().enumerate() {
        // Decide from the payments before this one; the position earns this one.
        let want = match mode {
            Mode::Always => true,
            Mode::PositiveTrailing { .. } => {
                i >= window && rates[i - window..i].iter().map(|x| x.rate).sum::<f64>() > 0.0
            }
        };
        if want != held {
            held = want;
            switches += 1;
            costs += cost;
            add(r.ts, -cost);
        }
        if held {
            held_n += 1;
            add(r.ts, 100.0 * r.rate * cfg.hedge_fraction);
        }
    }
    if held && let Some(last) = rates.last() {
        switches += 1;
        costs += cost;
        add(last.ts, -cost);
    }
    let series: Vec<f64> =
        (0..months.count).map(|k| monthly.get(&(months.first + k as i64)).copied().unwrap_or(0.0)).collect();
    let n = rates.len().max(1) as f64;
    CarryResult {
        total_pct: series.iter().sum(),
        monthly: series,
        switches,
        costs_pct: costs,
        negative_share: rates.iter().filter(|r| r.rate < 0.0).count() as f64 / n,
        held_share: held_n as f64 / n,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MONTH: i64 = 31 * 86_400_000;

    fn rates(v: &[f64]) -> Vec<FundingRate> {
        v.iter().enumerate().map(|(i, &rate)| FundingRate { ts: i as i64 * 8 * 3_600_000, rate }).collect()
    }

    #[test]
    fn always_on_collects_funding_minus_two_switches() {
        let cfg = CarryConfig::default();
        let r = run(&rates(&[0.0001; 90]), &cfg, Mode::Always, Months::between(0, MONTH));
        // 90 payments of 0.01% on half the account = 0.45%, less opening and closing.
        let expected = 90.0 * 0.01 * 0.5 - 2.0 * cfg.switch_cost_pct();
        assert!((r.total_pct - expected).abs() < 1e-9, "{} vs {expected}", r.total_pct);
        assert_eq!(r.switches, 2);
        assert_eq!(r.negative_share, 0.0);
    }

    #[test]
    fn the_filter_steps_aside_when_funding_turns_negative() {
        let mut v = vec![0.0001; 30];
        v.extend(vec![-0.0003; 30]);
        v.extend(vec![0.0001; 30]);
        let cfg = CarryConfig::default();
        let m = Months::between(0, MONTH);
        let always = run(&rates(&v), &cfg, Mode::Always, m);
        let filtered = run(&rates(&v), &cfg, Mode::PositiveTrailing { days: 1 }, m);
        assert!(filtered.total_pct > always.total_pct, "{} vs {}", filtered.total_pct, always.total_pct);
        assert!(filtered.held_share < 1.0);
        assert!(filtered.switches > 2);
    }
}
