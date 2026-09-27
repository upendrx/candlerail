//! Portfolio-level research that the single-market backtester can't express:
//! rotating between many coins, trading pairs, time-of-day effects and outside
//! signals such as a sentiment index.
//!
//! Every strategy here is a set of target weights. `w[s][t]` is decided at the
//! close of bar `t` and earns asset `s`'s return from `t` to `t + 1`; changing
//! a weight pays `cost_bps` on the change. Weights are fractions of the
//! account (1 = the whole account long, -0.5 = half of it short).

use crate::candle::Candle;
use crate::time;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Candles of many markets on one timeline. Missing bars are NaN, and a gap
/// in a market's data (a delisting and relisting) breaks its return series.
#[derive(Debug, Clone)]
pub struct Panel {
    pub ts: Vec<i64>,
    pub symbols: Vec<String>,
    /// `close[s][t]`
    pub close: Vec<Vec<f64>>,
}

impl Panel {
    pub fn new(series: &[(String, Vec<Candle>)]) -> Panel {
        let mut all: Vec<i64> = series.iter().flat_map(|(_, c)| c.iter().map(|x| x.ts)).collect();
        all.sort_unstable();
        all.dedup();
        let pos: BTreeMap<i64, usize> = all.iter().enumerate().map(|(i, &t)| (t, i)).collect();
        let close = series
            .iter()
            .map(|(_, c)| {
                let mut v = vec![f64::NAN; all.len()];
                for x in c {
                    v[pos[&x.ts]] = x.close;
                }
                v
            })
            .collect();
        Panel { ts: all, symbols: series.iter().map(|(s, _)| s.clone()).collect(), close }
    }

    pub fn index(&self, symbol: &str) -> Option<usize> {
        self.symbols.iter().position(|s| s == symbol)
    }

    /// `r[s][t]` = close[t] / close[t-1] − 1, NaN when either is missing.
    pub fn returns(&self) -> Vec<Vec<f64>> {
        self.close
            .iter()
            .map(|c| (0..c.len()).map(|t| if t == 0 { f64::NAN } else { c[t] / c[t - 1] - 1.0 }).collect())
            .collect()
    }

    /// For each bar, how many bars in a row the market has had a price.
    fn history(&self, s: usize) -> Vec<usize> {
        let mut out = Vec::with_capacity(self.ts.len());
        let mut run = 0;
        for &c in &self.close[s] {
            run = if c.is_finite() { run + 1 } else { 0 };
            out.push(run);
        }
        out
    }
}

/// The account's return at every bar.
#[derive(Debug, Clone, Default)]
pub struct Run {
    pub ts: Vec<i64>,
    pub ret: Vec<f64>,
    /// Total absolute weight changed, summed over the run.
    pub turnover: f64,
    /// Positions opened (a weight moving away from zero).
    pub trades: usize,
    /// Average gross exposure.
    pub exposure: f64,
}

/// Applies target weights to returns, paying costs on every change.
pub fn apply(r: &[Vec<f64>], ts: &[i64], w: &[Vec<f64>], cost_bps: f64) -> Run {
    let n = ts.len();
    let mut ret = vec![0.0; n];
    let (mut turnover, mut trades, mut gross) = (0.0, 0usize, 0.0);
    for t in 0..n {
        let mut cost = 0.0;
        for s in 0..w.len() {
            let now = w[s][t];
            let before = if t == 0 { 0.0 } else { w[s][t - 1] };
            let d = (now - before).abs();
            if d > 1e-12 {
                cost += d;
                if before.abs() < 1e-12 {
                    trades += 1;
                }
            }
            gross += now.abs();
            if t + 1 < n {
                let x = r[s][t + 1];
                if x.is_finite() {
                    ret[t + 1] += now * x;
                }
            }
        }
        turnover += cost;
        if t + 1 < n {
            ret[t + 1] -= cost * cost_bps / 10_000.0;
        }
    }
    Run { ts: ts.to_vec(), ret, turnover, trades, exposure: gross / n.max(1) as f64 }
}

/// Summary of part of a run.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct Stats {
    pub annual_pct: f64,
    pub vol_pct: f64,
    pub sharpe: f64,
    pub max_drawdown_pct: f64,
    pub months: usize,
    pub months_up: usize,
    pub avg_month_pct: f64,
    pub worst_month_pct: f64,
}

/// Month by month compounded return (%), from the month of `from`.
pub fn monthly(run: &Run, from: i64, to: i64) -> (i64, Vec<f64>) {
    let (m0, m1) = (time::month_index(from), time::month_index(to));
    let mut v = vec![1.0; (m1 - m0 + 1).max(0) as usize];
    for (t, &x) in run.ts.iter().zip(&run.ret) {
        if *t >= from && *t < to {
            v[(time::month_index(*t) - m0) as usize] *= 1.0 + x;
        }
    }
    (m0, v.into_iter().map(|g| 100.0 * (g - 1.0)).collect())
}

pub fn stats(run: &Run, from: i64, to: i64, bars_per_year: f64) -> Stats {
    let xs: Vec<f64> = run.ts.iter().zip(&run.ret).filter(|(t, _)| **t >= from && **t < to).map(|(_, x)| *x).collect();
    if xs.is_empty() {
        return Stats::default();
    }
    let n = xs.len() as f64;
    let mean = xs.iter().sum::<f64>() / n;
    let var = xs.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / n.max(2.0);
    let (mut eq, mut peak, mut dd) = (1.0f64, 1.0f64, 0.0f64);
    for x in &xs {
        eq *= 1.0 + x;
        peak = peak.max(eq);
        dd = dd.min(eq / peak - 1.0);
    }
    let (_, months) = monthly(run, from, to);
    let years = n / bars_per_year;
    Stats {
        annual_pct: 100.0 * (eq.max(1e-12).powf(1.0 / years.max(1e-9)) - 1.0),
        vol_pct: 100.0 * var.sqrt() * bars_per_year.sqrt(),
        sharpe: if var > 0.0 { mean / var.sqrt() * bars_per_year.sqrt() } else { 0.0 },
        max_drawdown_pct: 100.0 * dd,
        months: months.len(),
        months_up: months.iter().filter(|m| **m > 0.0).count(),
        avg_month_pct: months.iter().sum::<f64>() / months.len().max(1) as f64,
        worst_month_pct: months.iter().copied().fold(f64::INFINITY, f64::min).min(0.0),
    }
}

fn sma(v: &[f64], n: usize, t: usize) -> Option<f64> {
    if t + 1 < n {
        return None;
    }
    let w = &v[t + 1 - n..=t];
    w.iter().all(|x| x.is_finite()).then(|| w.iter().sum::<f64>() / n as f64)
}

/// Cross-sectional momentum: every `every` bars, rank markets by their return
/// over `lookback` bars and hold the strongest (and, long-short, short the
/// weakest).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Rotation {
    pub lookback: usize,
    pub every: usize,
    pub top: usize,
    pub long_short: bool,
    /// Only hold longs while BTC closes above its average of this many bars.
    pub btc_filter: Option<usize>,
    /// Bars of continuous history a market needs before it can be picked.
    pub min_history: usize,
    /// Rank the weakest markets instead (reversal rather than momentum).
    pub reverse: bool,
}

pub fn rotation_weights(p: &Panel, cfg: &Rotation) -> Vec<Vec<f64>> {
    let (ns, nt) = (p.symbols.len(), p.ts.len());
    let hist: Vec<Vec<usize>> = (0..ns).map(|s| p.history(s)).collect();
    let btc = p.index("BTCUSDT");
    let mut w = vec![vec![0.0; nt]; ns];
    let mut cur = vec![0.0; ns];
    for t in 0..nt {
        if t % cfg.every.max(1) == 0 {
            let mut ranked: Vec<(f64, usize)> = (0..ns)
                .filter(|&s| hist[s][t] > cfg.min_history.max(cfg.lookback))
                .map(|s| (p.close[s][t] / p.close[s][t - cfg.lookback] - 1.0, s))
                .filter(|(x, _)| x.is_finite())
                .collect();
            ranked.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
            if cfg.reverse {
                ranked.reverse();
            }
            cur = vec![0.0; ns];
            let k = cfg.top.min(ranked.len() / if cfg.long_short { 2 } else { 1 });
            let longs_on = match (cfg.btc_filter, btc) {
                (Some(n), Some(b)) => sma(&p.close[b], n, t).is_some_and(|m| p.close[b][t] > m),
                _ => true,
            };
            if k > 0 {
                let side = if cfg.long_short { 0.5 } else { 1.0 };
                if longs_on {
                    for &(_, s) in ranked.iter().take(k) {
                        cur[s] = side / k as f64;
                    }
                }
                if cfg.long_short {
                    for &(_, s) in ranked.iter().rev().take(k) {
                        cur[s] = -side / k as f64;
                    }
                }
            }
        }
        for s in 0..ns {
            // A market that stops trading is left at its last price.
            w[s][t] = if p.close[s][t].is_finite() { cur[s] } else { 0.0 };
        }
    }
    w
}

/// Market-neutral pairs: when the log price ratio of two markets is `entry`
/// standard deviations from its average over `window` bars, short the rich
/// one and buy the cheap one; close when it's back within `exit`, or after
/// `max_hold` bars.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Pair {
    pub window: usize,
    pub entry: f64,
    pub exit: f64,
    pub max_hold: usize,
}

/// Weights for markets `a` and `b`, each leg `size` of the account.
pub fn pair_weights(p: &Panel, a: usize, b: usize, cfg: &Pair, size: f64) -> (Vec<f64>, Vec<f64>) {
    let nt = p.ts.len();
    let spread: Vec<f64> = (0..nt).map(|t| (p.close[a][t] / p.close[b][t]).ln()).collect();
    let (mut wa, mut wb) = (vec![0.0; nt], vec![0.0; nt]);
    let (mut pos, mut held) = (0.0f64, 0usize);
    for t in 0..nt {
        if t + 1 >= cfg.window {
            let win = &spread[t + 1 - cfg.window..=t];
            if win.iter().all(|x| x.is_finite()) {
                let m = win.iter().sum::<f64>() / cfg.window as f64;
                let sd = (win.iter().map(|x| (x - m).powi(2)).sum::<f64>() / cfg.window as f64).sqrt();
                let z = if sd > 0.0 { (spread[t] - m) / sd } else { 0.0 };
                if pos == 0.0 {
                    if z > cfg.entry {
                        pos = -1.0;
                        held = 0;
                    } else if z < -cfg.entry {
                        pos = 1.0;
                        held = 0;
                    }
                } else {
                    held += 1;
                    if z.abs() < cfg.exit || held >= cfg.max_hold || z * pos > 0.0 && z.abs() > cfg.entry * 2.5 {
                        pos = 0.0;
                    }
                }
            } else {
                pos = 0.0;
            }
        }
        wa[t] = pos * size;
        wb[t] = -pos * size;
    }
    (wa, wb)
}

/// Correlation of two return series over the bars where both exist.
pub fn correlation(x: &[f64], y: &[f64], from: usize, to: usize) -> Option<f64> {
    let pairs: Vec<(f64, f64)> = (from..to.min(x.len()).min(y.len()))
        .map(|t| (x[t], y[t]))
        .filter(|(a, b)| a.is_finite() && b.is_finite())
        .collect();
    if pairs.len() < 30 {
        return None;
    }
    let n = pairs.len() as f64;
    let (mx, my) = (pairs.iter().map(|p| p.0).sum::<f64>() / n, pairs.iter().map(|p| p.1).sum::<f64>() / n);
    let (mut sxy, mut sxx, mut syy) = (0.0, 0.0, 0.0);
    for (a, b) in &pairs {
        sxy += (a - mx) * (b - my);
        sxx += (a - mx).powi(2);
        syy += (b - my).powi(2);
    }
    (sxx > 0.0 && syy > 0.0).then(|| sxy / (sxx * syy).sqrt())
}

/// A position that turns on when `enter` holds and off when `exit` does.
pub fn stateful(n: usize, side: f64, enter: impl Fn(usize) -> bool, exit: impl Fn(usize) -> bool) -> Vec<f64> {
    let mut w = vec![0.0; n];
    let mut on = false;
    for (t, x) in w.iter_mut().enumerate() {
        if on && exit(t) {
            on = false;
        } else if !on && enter(t) {
            on = true;
        }
        *x = if on { side } else { 0.0 };
    }
    w
}

/// Average return of the bar that starts at each hour of the UTC day (or
/// each weekday, with `weekday`), over [from, to).
pub fn seasonal_means(ts: &[i64], r: &[f64], from: i64, to: i64, weekday: bool) -> Vec<(f64, usize)> {
    let slots = if weekday { 7 } else { 24 };
    let mut acc = vec![(0.0, 0usize); slots];
    for (t, &x) in ts.iter().zip(r) {
        if *t >= from && *t < to && x.is_finite() {
            let k = slot(*t, weekday);
            acc[k].0 += x;
            acc[k].1 += 1;
        }
    }
    acc.into_iter().map(|(s, n)| (if n > 0 { s / n as f64 } else { 0.0 }, n)).collect()
}

/// Hour of the UTC day, or weekday with Monday as 0.
pub fn slot(ts: i64, weekday: bool) -> usize {
    if weekday { (time::day(ts) + 3).rem_euclid(7) as usize } else { (ts.rem_euclid(86_400_000) / 3_600_000) as usize }
}

/// One trade from a per-market backtest, for replaying through a shared account.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ReplayTrade {
    pub symbol: String,
    pub entry_ts: i64,
    pub exit_ts: i64,
    /// Result in units of the trade's risk, after costs.
    pub r: f64,
}

/// What happened when every market's signals shared one account.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct Replay {
    pub risk_pct: f64,
    pub max_open: usize,
    /// Profit month by month, as a % of the account at the start of the month.
    pub monthly: Vec<f64>,
    pub month0: i64,
    pub final_equity: f64,
    pub max_drawdown_pct: f64,
    pub annual_pct: f64,
    pub signals: usize,
    pub taken: usize,
    /// Signals skipped because `max_open` positions were already open.
    pub skipped: usize,
    pub max_open_seen: usize,
    pub avg_open: f64,
    /// Number of days with 0, 1, 2 ... new signals, over the whole period.
    pub signals_per_day: Vec<usize>,
}

/// Replays trades through one account: each trade risks `risk_pct` of the
/// account's value when it opens, at most `max_open` positions are held at
/// once, and results are booked when trades close. Starts at 1.
pub fn replay(trades: &[ReplayTrade], risk_pct: f64, max_open: usize, from: i64, to: i64) -> Replay {
    let mut t: Vec<&ReplayTrade> = trades.iter().filter(|x| x.entry_ts >= from && x.entry_ts < to).collect();
    t.sort_by_key(|x| (x.entry_ts, x.exit_ts));
    let mut equity = 1.0f64;
    let mut open: Vec<(i64, f64)> = vec![]; // (exit time, amount at risk)
    let (mut peak, mut dd, mut taken, mut skipped, mut max_seen) = (1.0f64, 0.0f64, 0usize, 0usize, 0usize);
    let (m0, m1) = (time::month_index(from), time::month_index(to));
    let mut month_end = vec![f64::NAN; (m1 - m0 + 1).max(0) as usize];
    let mut open_sum = 0.0;
    // Closed results, booked in time order.
    let mut closes: Vec<(i64, f64)> = vec![];
    for tr in &t {
        // Book everything that closed before this trade opens.
        closes.sort_by_key(|c| c.0);
        let mut k = 0;
        while k < closes.len() && closes[k].0 <= tr.entry_ts {
            equity += closes[k].1;
            peak = peak.max(equity);
            dd = dd.min(equity / peak - 1.0);
            let mi = (time::month_index(closes[k].0) - m0) as usize;
            if mi < month_end.len() {
                month_end[mi] = equity;
            }
            k += 1;
        }
        closes.drain(..k);
        open.retain(|(exit, _)| *exit > tr.entry_ts);
        if open.len() >= max_open {
            skipped += 1;
            continue;
        }
        let risk = equity * risk_pct / 100.0;
        open.push((tr.exit_ts, risk));
        max_seen = max_seen.max(open.len());
        open_sum += open.len() as f64;
        taken += 1;
        closes.push((tr.exit_ts, tr.r * risk));
    }
    closes.sort_by_key(|c| c.0);
    for (ts, pnl) in closes {
        equity += pnl;
        peak = peak.max(equity);
        dd = dd.min(equity / peak - 1.0);
        let mi = (time::month_index(ts) - m0) as usize;
        if mi < month_end.len() {
            month_end[mi] = equity;
        }
    }
    // Month-end equity, carried forward through months without closes.
    let mut last = 1.0;
    let mut monthly = Vec::with_capacity(month_end.len());
    for e in month_end {
        let now = if e.is_finite() { e } else { last };
        monthly.push(100.0 * (now / last - 1.0));
        last = now;
    }
    let mut per_day: BTreeMap<i64, usize> = BTreeMap::new();
    for tr in &t {
        *per_day.entry(tr.entry_ts.div_euclid(86_400_000)).or_insert(0) += 1;
    }
    let days = ((to - from) / 86_400_000).max(1) as usize;
    let mut hist = vec![0usize; per_day.values().copied().max().unwrap_or(0) + 1];
    hist[0] = days.saturating_sub(per_day.len());
    for n in per_day.values() {
        hist[*n] += 1;
    }
    let years = (to - from) as f64 / (365.0 * 86_400_000.0);
    Replay {
        risk_pct,
        max_open,
        monthly,
        month0: m0,
        final_equity: equity,
        max_drawdown_pct: 100.0 * dd,
        annual_pct: 100.0 * (equity.max(1e-12).powf(1.0 / years.max(1e-9)) - 1.0),
        signals: t.len(),
        taken,
        skipped,
        max_open_seen: max_seen,
        avg_open: if taken > 0 { open_sum / taken as f64 } else { 0.0 },
        signals_per_day: hist,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn candles(prices: &[f64]) -> Vec<Candle> {
        prices
            .iter()
            .enumerate()
            .map(|(i, &c)| Candle { ts: i as i64 * 86_400_000, open: c, high: c, low: c, close: c, volume: 1.0 })
            .collect()
    }

    #[test]
    fn weights_earn_the_next_bar_and_pay_costs() {
        let p = Panel::new(&[("A".into(), candles(&[100.0, 110.0, 121.0]))]);
        let r = p.returns();
        let run = apply(&r, &p.ts, &[vec![1.0, 1.0, 0.0]], 10.0);
        // Bought at the first close: +10% twice, minus 0.1% to buy and 0.1% to sell.
        assert!((run.ret[1] - (0.10 - 0.001)).abs() < 1e-12);
        assert!((run.ret[2] - 0.10).abs() < 1e-12);
        assert_eq!(run.trades, 1);
        assert!((run.turnover - 2.0).abs() < 1e-12);
    }

    #[test]
    fn rotation_holds_the_strongest_and_skips_new_listings() {
        let up: Vec<f64> = (0..40).map(|i| 100.0 * 1.01f64.powi(i)).collect();
        let down: Vec<f64> = (0..40).map(|i| 100.0 * 0.99f64.powi(i)).collect();
        let mut late = vec![f64::NAN; 30];
        late.extend((0..10).map(|i| 100.0 * 1.2f64.powi(i)));
        let late_c: Vec<Candle> = candles(&late).into_iter().filter(|c| c.close.is_finite()).collect();
        let p = Panel::new(&[("UP".into(), candles(&up)), ("DOWN".into(), candles(&down)), ("NEW".into(), late_c)]);
        let cfg = Rotation {
            lookback: 5,
            every: 5,
            top: 1,
            long_short: true,
            btc_filter: None,
            min_history: 20,
            reverse: false,
        };
        let w = rotation_weights(&p, &cfg);
        assert_eq!(w[0][35], 0.5, "long the strongest");
        assert_eq!(w[1][35], -0.5, "short the weakest");
        assert_eq!(w[2][35], 0.0, "too new to pick");
    }

    #[test]
    fn pairs_fade_a_stretched_ratio() {
        let a: Vec<f64> = (0..60).map(|i| if i == 50 { 130.0 } else { 100.0 + (i % 2) as f64 }).collect();
        let b: Vec<f64> = (0..60).map(|_| 100.0).collect();
        let p = Panel::new(&[("A".into(), candles(&a)), ("B".into(), candles(&b))]);
        let (wa, wb) = pair_weights(&p, 0, 1, &Pair { window: 20, entry: 2.0, exit: 0.5, max_hold: 5 }, 0.5);
        assert_eq!(wa[50], -0.5, "A is rich: short it");
        assert_eq!(wb[50], 0.5);
        assert_eq!(wa[40], 0.0);
    }

    #[test]
    fn seasonal_slots() {
        // 1970-01-05 was a Monday.
        assert_eq!(slot(4 * 86_400_000, true), 0);
        assert_eq!(slot(3 * 3_600_000, false), 3);
    }

    #[test]
    fn replay_sizes_from_the_account_and_caps_open_positions() {
        let d = 86_400_000;
        let t = |s: &str, a: i64, b: i64, r: f64| ReplayTrade { symbol: s.into(), entry_ts: a * d, exit_ts: b * d, r };
        let trades = vec![t("A", 0, 5, 2.0), t("B", 1, 6, -1.0), t("C", 2, 3, 1.0), t("D", 7, 8, 1.0)];
        let r = replay(&trades, 1.0, 2, 0, 30 * d);
        assert_eq!((r.taken, r.skipped, r.max_open_seen), (3, 1, 2), "C is skipped: A and B are open");
        // +2% and -1% on a 1.0 account, then D risks 1% of 1.01.
        assert!((r.final_equity - (1.01 + 0.0101)).abs() < 1e-12, "{}", r.final_equity);
        assert_eq!(r.signals_per_day[1], 4);
    }
}
