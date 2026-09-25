use super::Indicator;
use super::primitives::{Ema, MinMax, Rma, Sma};
use crate::candle::Candle;
use std::collections::VecDeque;

/// RSI with Wilder smoothing of gains and losses.
pub struct Rsi {
    prev: Option<f64>,
    gain: Rma,
    loss: Rma,
}

impl Rsi {
    pub fn new(period: usize) -> Self {
        Rsi { prev: None, gain: Rma::new(period), loss: Rma::new(period) }
    }
}

impl Indicator for Rsi {
    fn update(&mut self, c: &Candle) {
        if let Some(p) = self.prev.replace(c.close) {
            let d = c.close - p;
            self.gain.update(d.max(0.0));
            self.loss.update((-d).max(0.0));
        }
    }

    fn get(&self, _: usize) -> Option<f64> {
        let (g, l) = (self.gain.value()?, self.loss.value()?);
        Some(if l == 0.0 { if g == 0.0 { 50.0 } else { 100.0 } } else { 100.0 - 100.0 / (1.0 + g / l) })
    }
}

pub struct Macd {
    fast: Ema,
    slow: Ema,
    signal: Ema,
    line: Option<f64>,
}

impl Macd {
    pub fn new(fast: usize, slow: usize, signal: usize) -> Self {
        Macd { fast: Ema::new(fast), slow: Ema::new(slow), signal: Ema::new(signal), line: None }
    }
}

impl Indicator for Macd {
    fn update(&mut self, c: &Candle) {
        let f = self.fast.update(c.close);
        let s = self.slow.update(c.close);
        if let (Some(f), Some(s)) = (f, s) {
            self.line = Some(f - s);
            self.signal.update(f - s);
        }
    }

    fn get(&self, i: usize) -> Option<f64> {
        match i {
            0 => self.line,
            1 => self.signal.value(),
            _ => Some(self.line? - self.signal.value()?),
        }
    }
}

/// Slow stochastic: raw %K smoothed by `k_smooth`, %D = SMA of %K.
pub struct Stoch {
    range: MinMax,
    k: Sma,
    d: Sma,
}

impl Stoch {
    pub fn new(k_period: usize, k_smooth: usize, d_period: usize) -> Self {
        Stoch { range: MinMax::new(k_period), k: Sma::new(k_smooth), d: Sma::new(d_period) }
    }
}

impl Indicator for Stoch {
    fn update(&mut self, c: &Candle) {
        self.range.update(c.high, c.low);
        let Some((hi, lo)) = self.range.value() else { return };
        let raw = if hi > lo { 100.0 * (c.close - lo) / (hi - lo) } else { 50.0 };
        if let Some(k) = self.k.update(raw) {
            self.d.update(k);
        }
    }

    fn get(&self, i: usize) -> Option<f64> {
        if i == 0 { self.k.value() } else { self.d.value() }
    }
}

pub struct Roc {
    period: usize,
    closes: VecDeque<f64>,
}

impl Roc {
    pub fn new(period: usize) -> Self {
        Roc { period, closes: VecDeque::with_capacity(period + 2) }
    }
}

impl Indicator for Roc {
    fn update(&mut self, c: &Candle) {
        self.closes.push_back(c.close);
        if self.closes.len() > self.period + 1 {
            self.closes.pop_front();
        }
    }

    fn get(&self, _: usize) -> Option<f64> {
        if self.closes.len() <= self.period {
            return None;
        }
        let (old, new) = (*self.closes.front()?, *self.closes.back()?);
        (old != 0.0).then(|| 100.0 * (new / old - 1.0))
    }
}
