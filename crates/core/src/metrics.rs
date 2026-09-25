//! Backtest statistics, and the warnings that go with them.

use crate::backtest::{BacktestConfig, EquityPoint};
use crate::broker::{ExitReason, Trade};
use crate::candle::Candle;
use crate::spec::Strategy;
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct Metrics {
    pub start_equity: f64,
    pub final_equity: f64,
    pub total_return_pct: f64,
    /// Return of simply buying at the first open and holding (no leverage, no fees).
    pub buy_hold_return_pct: f64,
    pub max_drawdown_pct: f64,
    pub trades: usize,
    pub win_rate_pct: f64,
    /// Gross profit / gross loss. `None` when there were no losing trades.
    pub profit_factor: Option<f64>,
    pub avg_win: f64,
    pub avg_loss: f64,
    /// Average PnL per trade.
    pub expectancy: f64,
    pub best_trade: f64,
    pub worst_trade: f64,
    pub avg_bars_held: f64,
    /// Share of bars with a position open.
    pub exposure_pct: f64,
    /// Annualised Sharpe ratio of per-bar equity returns (no risk-free rate).
    pub sharpe: f64,
    pub liquidations: usize,
    pub fees_paid: f64,
    pub longest_losing_streak: usize,
    /// Entries that couldn't be sized.
    pub skipped_entries: u32,
    /// Average result per trade in units of initial risk (needs a stop-loss).
    pub avg_r: Option<f64>,
    /// Kelly estimate of the % of equity to risk per trade, from the win rate and
    /// the average win and loss in R. Most traders use a half or less.
    pub kelly_risk_pct: Option<f64>,
}

/// Summary of part of the test period.
#[derive(Debug, Clone, Serialize)]
pub struct Segment {
    pub return_pct: f64,
    pub trades: usize,
    pub win_rate_pct: f64,
    pub profit_factor: Option<f64>,
}

fn profit_factor(trades: &[&Trade]) -> Option<f64> {
    let gp: f64 = trades.iter().filter(|t| t.pnl > 0.0).map(|t| t.pnl).sum();
    let gl: f64 = -trades.iter().filter(|t| t.pnl < 0.0).map(|t| t.pnl).sum::<f64>();
    (gl > 0.0).then(|| gp / gl)
}

fn win_rate(trades: &[&Trade]) -> f64 {
    if trades.is_empty() {
        0.0
    } else {
        100.0 * trades.iter().filter(|t| t.pnl > 0.0).count() as f64 / trades.len() as f64
    }
}

pub fn compute(
    trades: &[Trade],
    equity: &[EquityPoint],
    candles: &[Candle],
    cfg: &BacktestConfig,
    bars_in_market: usize,
    skipped: u32,
) -> Metrics {
    let all: Vec<&Trade> = trades.iter().collect();
    let wins: Vec<f64> = trades.iter().filter(|t| t.pnl > 0.0).map(|t| t.pnl).collect();
    let losses: Vec<f64> = trades.iter().filter(|t| t.pnl <= 0.0).map(|t| t.pnl).collect();
    let mean = |v: &[f64]| if v.is_empty() { 0.0 } else { v.iter().sum::<f64>() / v.len() as f64 };
    let final_equity = equity.last().map_or(cfg.capital, |e| e.equity);

    let mut peak = cfg.capital;
    let mut max_dd: f64 = 0.0;
    for e in equity {
        peak = peak.max(e.equity);
        if peak > 0.0 {
            max_dd = max_dd.max(100.0 * (peak - e.equity) / peak);
        }
    }

    let rets: Vec<f64> =
        equity.windows(2).filter(|w| w[0].equity > 0.0).map(|w| w[1].equity / w[0].equity - 1.0).collect();
    let mu = mean(&rets);
    let sd = (rets.iter().map(|r| (r - mu) * (r - mu)).sum::<f64>() / rets.len().max(1) as f64).sqrt();
    let sharpe = if sd > 0.0 { mu / sd * cfg.interval.bars_per_year().sqrt() } else { 0.0 };

    let mut streak = 0;
    let mut longest = 0;
    for t in trades {
        if t.pnl <= 0.0 {
            streak += 1;
            longest = longest.max(streak);
        } else {
            streak = 0;
        }
    }

    let rs: Vec<f64> = trades.iter().filter_map(|t| t.r_multiple).collect();
    let avg_r = (!rs.is_empty()).then(|| mean(&rs));
    let kelly_risk_pct = (rs.len() >= 10)
        .then(|| {
            let wins: Vec<f64> = rs.iter().copied().filter(|r| *r > 0.0).collect();
            let losses: Vec<f64> = rs.iter().filter(|r| **r <= 0.0).map(|r| -r).collect();
            let (w, l) = (mean(&wins), mean(&losses));
            if wins.is_empty() {
                return Some(0.0);
            }
            if losses.is_empty() || l <= 0.0 {
                return None;
            }
            let p = wins.len() as f64 / rs.len() as f64;
            Some(100.0 * (p - (1.0 - p) / (w / l)))
        })
        .flatten();
    let first = candles.first().map_or(1.0, |c| c.open);
    let last = candles.last().map_or(1.0, |c| c.close);
    Metrics {
        start_equity: cfg.capital,
        final_equity,
        total_return_pct: 100.0 * (final_equity / cfg.capital - 1.0),
        buy_hold_return_pct: if first > 0.0 { 100.0 * (last / first - 1.0) } else { 0.0 },
        max_drawdown_pct: max_dd,
        trades: trades.len(),
        win_rate_pct: win_rate(&all),
        profit_factor: profit_factor(&all),
        avg_win: mean(&wins),
        avg_loss: mean(&losses),
        expectancy: mean(&trades.iter().map(|t| t.pnl).collect::<Vec<_>>()),
        best_trade: trades.iter().map(|t| t.pnl).fold(0.0, f64::max),
        worst_trade: trades.iter().map(|t| t.pnl).fold(0.0, f64::min),
        avg_bars_held: mean(&trades.iter().map(|t| t.bars as f64).collect::<Vec<_>>()),
        exposure_pct: if candles.is_empty() { 0.0 } else { 100.0 * bars_in_market as f64 / candles.len() as f64 },
        sharpe,
        liquidations: trades.iter().filter(|t| t.reason == ExitReason::Liquidation).count(),
        fees_paid: trades.iter().map(|t| t.fees).sum(),
        longest_losing_streak: longest,
        skipped_entries: skipped,
        avg_r,
        kelly_risk_pct,
    }
}

pub fn segment(trades: &[Trade], equity: &[EquityPoint], include: impl Fn(&Trade) -> bool) -> Segment {
    let ts: Vec<&Trade> = trades.iter().filter(|t| include(t)).collect();
    let ret = match (equity.first(), equity.last()) {
        (Some(a), Some(b)) if a.equity > 0.0 => 100.0 * (b.equity / a.equity - 1.0),
        _ => 0.0,
    };
    Segment { return_pct: ret, trades: ts.len(), win_rate_pct: win_rate(&ts), profit_factor: profit_factor(&ts) }
}

pub fn warnings(s: &Strategy, m: &Metrics, is: &Segment, oos: &Segment, cfg: &BacktestConfig) -> Vec<String> {
    let mut w = vec![];
    if m.trades == 0 {
        w.push("No trades. The entry rules never triggered on this data; loosen them or test a longer period.".into());
    } else if m.trades < 30 {
        w.push(format!(
            "Only {} trades, which is too few to tell skill from luck. Test a longer period or other markets before trusting this.",
            m.trades
        ));
    }
    if m.liquidations > 0 {
        w.push(format!(
            "{} trade(s) were liquidated at {}x leverage, losing their whole margin. Lower the leverage or tighten the stop.",
            m.liquidations, s.leverage
        ));
    }
    if m.max_drawdown_pct > 30.0 {
        w.push(format!(
            "The account fell {:.0}% from its peak at worst. Most people abandon a strategy long before a drawdown that deep.",
            m.max_drawdown_pct
        ));
    }
    let pf_is = is.profit_factor.unwrap_or(if is.trades > 0 { f64::INFINITY } else { 0.0 });
    let pf_oos = oos.profit_factor.unwrap_or(if oos.trades > 0 { f64::INFINITY } else { 0.0 });
    if is.trades >= 5 && oos.trades >= 5 && pf_is > 1.2 && pf_oos < 1.0 {
        w.push(format!(
            "Profitable on the first {:.0}% of the data but losing on the last {:.0}%. That pattern usually means the rules were fitted to the past.",
            cfg.split * 100.0,
            (1.0 - cfg.split) * 100.0
        ));
    }
    let gross_profit: f64 = m.avg_win * (m.trades as f64 * m.win_rate_pct / 100.0);
    if gross_profit > 0.0 && m.fees_paid > 0.3 * gross_profit {
        w.push(format!(
            "Fees were {:.0}% of gross profit. Trade less often, or check your exchange's fee tier.",
            100.0 * m.fees_paid / gross_profit
        ));
    }
    if m.trades > 0 && m.total_return_pct < m.buy_hold_return_pct && m.buy_hold_return_pct > 0.0 {
        w.push(format!(
            "Simply buying and holding returned {:.1}% over the same period, versus {:.1}% here.",
            m.buy_hold_return_pct, m.total_return_pct
        ));
    }
    if s.leverage > 5.0 {
        w.push(format!(
            "At {}x leverage a {:.1}% move against a position wipes out its margin. Small moves decide everything.",
            s.leverage,
            100.0 / s.leverage
        ));
    }
    if let Some(k) = m.kelly_risk_pct {
        if k <= 0.0 {
            w.push("By the Kelly measure this strategy has no edge: the win rate and payoff don't support risking anything per trade.".into());
        } else if s.sizing.kind == crate::spec::SizingType::RiskPercent && s.sizing.value > k / 2.0 {
            w.push(format!(
                "You risk {}% per trade, more than half-Kelly ({:.1}%) for these results. Many traders stay at or below half-Kelly.",
                s.sizing.value,
                k / 2.0
            ));
        }
    }
    if m.skipped_entries > 0 {
        w.push(format!(
            "{} entries were skipped because the position couldn't be sized with the equity left.",
            m.skipped_entries
        ));
    }
    w
}
