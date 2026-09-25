//! Building blocks for price-action trading: candle patterns, swing-based
//! support and resistance, higher-timeframe levels, opening ranges and volume.
//! None of these smooth or transform price; they describe it.

use super::Indicator;
use super::primitives::Sma;
use crate::candle::Candle;
use crate::time;
use std::collections::VecDeque;

fn parts(c: &Candle) -> (f64, f64, f64, f64) {
    let body = (c.close - c.open).abs();
    let range = c.high - c.low;
    let upper = c.high - c.open.max(c.close);
    let lower = c.open.min(c.close) - c.low;
    (body, range, upper, lower)
}

fn bull(c: &Candle) -> bool {
    c.close > c.open
}

fn bear(c: &Candle) -> bool {
    c.close < c.open
}

/// Output order for [`Patterns`]; must match the catalog.
pub const PATTERN_NAMES: [&str; 13] = [
    "bullish_engulfing",
    "bearish_engulfing",
    "hammer",
    "shooting_star",
    "doji",
    "inside_bar",
    "outside_bar",
    "morning_star",
    "evening_star",
    "three_white_soldiers",
    "three_black_crows",
    "bullish_marubozu",
    "bearish_marubozu",
];

/// Classic candlestick patterns. Every output is 1 on the candle that completes
/// the pattern and 0 otherwise.
pub struct Patterns {
    bars: VecDeque<Candle>,
    wick_ratio: f64,
    doji_ratio: f64,
    out: [f64; 13],
    ready: bool,
}

impl Patterns {
    pub fn new(wick_ratio: f64, doji_percent: f64) -> Self {
        Patterns {
            bars: VecDeque::with_capacity(4),
            wick_ratio,
            doji_ratio: doji_percent / 100.0,
            out: [0.0; 13],
            ready: false,
        }
    }

    fn detect(&self) -> [f64; 13] {
        let n = self.bars.len();
        let c0 = &self.bars[n - 1];
        let (b0, r0, u0, l0) = parts(c0);
        let flag = |x: bool| if x { 1.0 } else { 0.0 };
        let mut o = [0.0; 13];
        if r0 > 0.0 {
            let small_body = b0.max(0.05 * r0);
            o[2] = flag(l0 >= self.wick_ratio * small_body && u0 <= 0.25 * r0);
            o[3] = flag(u0 >= self.wick_ratio * small_body && l0 <= 0.25 * r0);
            o[4] = flag(b0 <= self.doji_ratio * r0);
            o[11] = flag(bull(c0) && b0 >= 0.9 * r0);
            o[12] = flag(bear(c0) && b0 >= 0.9 * r0);
        }
        if n >= 2 {
            let c1 = &self.bars[n - 2];
            let b1 = (c1.close - c1.open).abs();
            o[0] = flag(bear(c1) && bull(c0) && c0.open <= c1.close && c0.close >= c1.open && b0 > b1);
            o[1] = flag(bull(c1) && bear(c0) && c0.open >= c1.close && c0.close <= c1.open && b0 > b1);
            o[5] = flag(c0.high <= c1.high && c0.low >= c1.low && (c0.high < c1.high || c0.low > c1.low));
            o[6] = flag(c0.high >= c1.high && c0.low <= c1.low && (c0.high > c1.high || c0.low < c1.low));
        }
        if n >= 3 {
            let (c2, c1) = (&self.bars[n - 3], &self.bars[n - 2]);
            let (b2, r2, _, _) = parts(c2);
            let (b1, r1, _, _) = parts(c1);
            let mid2 = (c2.open + c2.close) / 2.0;
            o[7] = flag(bear(c2) && b2 >= 0.5 * r2 && b1 <= 0.5 * b2 && bull(c0) && c0.close > mid2);
            o[8] = flag(bull(c2) && b2 >= 0.5 * r2 && b1 <= 0.5 * b2 && bear(c0) && c0.close < mid2);
            let strong = |b: f64, r: f64| r > 0.0 && b >= 0.5 * r;
            o[9] = flag(
                bull(c2)
                    && bull(c1)
                    && bull(c0)
                    && c1.close > c2.close
                    && c0.close > c1.close
                    && c1.open > c2.open
                    && c1.open <= c2.close
                    && c0.open > c1.open
                    && c0.open <= c1.close
                    && strong(b2, r2)
                    && strong(b1, r1)
                    && strong(b0, r0),
            );
            o[10] = flag(
                bear(c2)
                    && bear(c1)
                    && bear(c0)
                    && c1.close < c2.close
                    && c0.close < c1.close
                    && c1.open < c2.open
                    && c1.open >= c2.close
                    && c0.open < c1.open
                    && c0.open >= c1.close
                    && strong(b2, r2)
                    && strong(b1, r1)
                    && strong(b0, r0),
            );
        }
        o
    }
}

impl Indicator for Patterns {
    fn update(&mut self, c: &Candle) {
        self.bars.push_back(*c);
        if self.bars.len() > 3 {
            self.bars.pop_front();
        }
        self.out = self.detect();
        self.ready = true;
    }

    fn get(&self, i: usize) -> Option<f64> {
        self.ready.then(|| self.out[i])
    }
}

/// Swing highs and lows (fractals). A swing high is a candle whose high is above
/// the `left` candles before it and not exceeded by the `right` candles after it,
/// so it's only known `right` candles later. Nothing here looks ahead.
pub struct Swings {
    left: usize,
    right: usize,
    buf: VecDeque<Candle>,
    res: Option<f64>,
    sup: Option<f64>,
    prev_res: Option<f64>,
    prev_sup: Option<f64>,
}

impl Swings {
    pub fn new(left: usize, right: usize) -> Self {
        Swings {
            left: left.max(1),
            right: right.max(1),
            buf: VecDeque::with_capacity(left + right + 2),
            res: None,
            sup: None,
            prev_res: None,
            prev_sup: None,
        }
    }
}

impl Indicator for Swings {
    fn update(&mut self, c: &Candle) {
        self.buf.push_back(*c);
        let n = self.left + self.right + 1;
        if self.buf.len() > n {
            self.buf.pop_front();
        }
        if self.buf.len() < n {
            return;
        }
        let pivot = self.buf[self.left];
        let before = self.buf.iter().take(self.left);
        let after = self.buf.iter().skip(self.left + 1);
        if before.clone().all(|b| pivot.high > b.high) && after.clone().all(|b| pivot.high >= b.high) {
            self.prev_res = self.res;
            self.res = Some(pivot.high);
        }
        if before.clone().all(|b| pivot.low < b.low) && after.clone().all(|b| pivot.low <= b.low) {
            self.prev_sup = self.sup;
            self.sup = Some(pivot.low);
        }
    }

    fn get(&self, i: usize) -> Option<f64> {
        match i {
            0 => self.res,
            1 => self.sup,
            2 => self.prev_res,
            3 => self.prev_sup,
            // Market structure: 1 = higher high and higher low, -1 = lower high and lower low.
            _ => {
                let (r, s, pr, ps) = (self.res?, self.sup?, self.prev_res?, self.prev_sup?);
                Some(if r > pr && s > ps {
                    1.0
                } else if r < pr && s < ps {
                    -1.0
                } else {
                    0.0
                })
            }
        }
    }
}

/// Levels of a higher timeframe: the current period's open, high and low so
/// far, and the previous period's open, high, low and close. `minutes` of 1440
/// is a UTC day, 10080 a week starting Monday, and 43200 a calendar month.
pub struct Period {
    minutes: u64,
    key: Option<i64>,
    cur: Option<(f64, f64, f64, f64)>,
    prev: Option<(f64, f64, f64, f64)>,
}

impl Period {
    pub fn new(minutes: u64) -> Self {
        Period { minutes: minutes.max(1), key: None, cur: None, prev: None }
    }

    fn key_of(&self, ts: i64) -> i64 {
        match self.minutes {
            43_200 => time::month_index(ts),
            // 1970-01-05 was a Monday: shift so weeks start on Monday.
            10_080 => (time::day(ts) - 4).div_euclid(7),
            m => ts.div_euclid(m as i64 * 60_000),
        }
    }
}

impl Indicator for Period {
    fn update(&mut self, c: &Candle) {
        let k = self.key_of(c.ts);
        if self.key != Some(k) {
            self.key = Some(k);
            self.prev = self.cur;
            self.cur = Some((c.open, c.high, c.low, c.close));
        } else if let Some(p) = &mut self.cur {
            p.1 = p.1.max(c.high);
            p.2 = p.2.min(c.low);
            p.3 = c.close;
        }
    }

    fn get(&self, i: usize) -> Option<f64> {
        match i {
            0 => self.cur.map(|p| p.0),
            1 => self.cur.map(|p| p.1),
            2 => self.cur.map(|p| p.2),
            3 => self.prev.map(|p| p.0),
            4 => self.prev.map(|p| p.1),
            5 => self.prev.map(|p| p.2),
            _ => self.prev.map(|p| p.3),
        }
    }
}

/// High and low of the first `minutes` of each session. `session_start` is the
/// session open in minutes after 00:00 UTC (0 for crypto; 810 is 13:30 UTC,
/// the New York open in summer). `ready` turns 1 once the range is complete.
pub struct OpeningRange {
    window_ms: i64,
    start_ms: i64,
    day: Option<i64>,
    high: Option<f64>,
    low: Option<f64>,
    ready: bool,
}

impl OpeningRange {
    pub fn new(minutes: u64, session_start: u64) -> Self {
        OpeningRange {
            window_ms: minutes as i64 * 60_000,
            start_ms: session_start as i64 * 60_000,
            day: None,
            high: None,
            low: None,
            ready: false,
        }
    }
}

impl Indicator for OpeningRange {
    fn update(&mut self, c: &Candle) {
        let since = c.ts - self.start_ms;
        let day = since.div_euclid(86_400_000);
        if self.day != Some(day) {
            self.day = Some(day);
            self.high = None;
            self.low = None;
            self.ready = false;
        }
        if since.rem_euclid(86_400_000) < self.window_ms {
            self.high = Some(self.high.map_or(c.high, |h| h.max(c.high)));
            self.low = Some(self.low.map_or(c.low, |l| l.min(c.low)));
        } else if self.high.is_some() {
            self.ready = true;
        }
    }

    fn get(&self, i: usize) -> Option<f64> {
        match i {
            0 => self.high,
            1 => self.low,
            _ => self.high.map(|_| if self.ready { 1.0 } else { 0.0 }),
        }
    }
}

/// Average volume of the previous `period` candles, and this candle's volume
/// relative to it (2 = twice the usual volume).
pub struct VolumeAvg {
    sma: Sma,
    avg: Option<f64>,
    rel: Option<f64>,
}

impl VolumeAvg {
    pub fn new(period: usize) -> Self {
        VolumeAvg { sma: Sma::new(period), avg: None, rel: None }
    }
}

impl Indicator for VolumeAvg {
    fn update(&mut self, c: &Candle) {
        // Compare with the average *before* this candle, so a spike doesn't dilute itself.
        self.rel = self.sma.value().filter(|a| *a > 0.0).map(|a| c.volume / a);
        self.avg = self.sma.update(c.volume);
    }

    fn get(&self, i: usize) -> Option<f64> {
        if i == 0 { self.avg } else { self.rel }
    }
}
