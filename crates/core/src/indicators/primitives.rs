//! Moving-average building blocks shared by the indicators.

use std::collections::VecDeque;

/// Simple moving average over a fixed window.
#[derive(Clone, Debug)]
pub struct Sma {
    period: usize,
    buf: VecDeque<f64>,
    sum: f64,
}

impl Sma {
    pub fn new(period: usize) -> Self {
        Sma { period: period.max(1), buf: VecDeque::with_capacity(period + 1), sum: 0.0 }
    }

    pub fn update(&mut self, v: f64) -> Option<f64> {
        self.buf.push_back(v);
        self.sum += v;
        if self.buf.len() > self.period {
            self.sum -= self.buf.pop_front().unwrap_or(0.0);
        }
        self.value()
    }

    pub fn value(&self) -> Option<f64> {
        (self.buf.len() == self.period).then(|| self.sum / self.period as f64)
    }

    /// Population standard deviation of the current window.
    pub fn stddev(&self) -> Option<f64> {
        let m = self.value()?;
        let var = self.buf.iter().map(|x| (x - m) * (x - m)).sum::<f64>() / self.period as f64;
        Some(var.sqrt())
    }
}

/// Exponential moving average, seeded with the SMA of the first `period` values
/// (the usual convention, and what most charting tools do).
#[derive(Clone, Debug)]
pub struct Ema {
    alpha: f64,
    seed: Sma,
    value: Option<f64>,
}

impl Ema {
    pub fn new(period: usize) -> Self {
        Self::with_alpha(period, 2.0 / (period.max(1) as f64 + 1.0))
    }

    fn with_alpha(period: usize, alpha: f64) -> Self {
        Ema { alpha, seed: Sma::new(period), value: None }
    }

    pub fn update(&mut self, v: f64) -> Option<f64> {
        self.value = match self.value {
            Some(prev) => Some(prev + self.alpha * (v - prev)),
            None => self.seed.update(v),
        };
        self.value
    }

    pub fn value(&self) -> Option<f64> {
        self.value
    }
}

/// Wilder's smoothing (RMA): an EMA with alpha = 1/period. Used by RSI, ATR and ADX.
#[derive(Clone, Debug)]
pub struct Rma(Ema);

impl Rma {
    pub fn new(period: usize) -> Self {
        Rma(Ema::with_alpha(period, 1.0 / period.max(1) as f64))
    }

    pub fn update(&mut self, v: f64) -> Option<f64> {
        self.0.update(v)
    }

    pub fn value(&self) -> Option<f64> {
        self.0.value()
    }
}

/// Rolling highest and lowest value over a window.
#[derive(Clone, Debug)]
pub struct MinMax {
    period: usize,
    buf: VecDeque<(f64, f64)>,
}

impl MinMax {
    pub fn new(period: usize) -> Self {
        MinMax { period: period.max(1), buf: VecDeque::with_capacity(period + 1) }
    }

    pub fn update(&mut self, high: f64, low: f64) {
        self.buf.push_back((high, low));
        if self.buf.len() > self.period {
            self.buf.pop_front();
        }
    }

    /// (highest high, lowest low) once the window is full.
    pub fn value(&self) -> Option<(f64, f64)> {
        if self.buf.len() < self.period {
            return None;
        }
        let hi = self.buf.iter().map(|x| x.0).fold(f64::MIN, f64::max);
        let lo = self.buf.iter().map(|x| x.1).fold(f64::MAX, f64::min);
        Some((hi, lo))
    }
}

/// True range of a bar given the previous close.
pub fn true_range(high: f64, low: f64, prev_close: Option<f64>) -> f64 {
    match prev_close {
        Some(pc) => (high - low).max((high - pc).abs()).max((low - pc).abs()),
        None => high - low,
    }
}
