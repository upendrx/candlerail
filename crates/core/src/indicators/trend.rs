use super::Indicator;
use super::primitives::{Ema, Rma, Sma, true_range};
use crate::candle::Candle;

pub struct SmaInd(pub Sma);

impl Indicator for SmaInd {
    fn update(&mut self, c: &Candle) {
        self.0.update(c.close);
    }
    fn get(&self, _: usize) -> Option<f64> {
        self.0.value()
    }
}

pub struct EmaInd(pub Ema);

impl Indicator for EmaInd {
    fn update(&mut self, c: &Candle) {
        self.0.update(c.close);
    }
    fn get(&self, _: usize) -> Option<f64> {
        self.0.value()
    }
}

/// SuperTrend: bands at hl2 ± multiplier × ATR that only ratchet in the trend's
/// favour; the trend flips when the close crosses the active band.
pub struct SuperTrend {
    atr: Rma,
    mult: f64,
    prev_close: Option<f64>,
    upper: Option<f64>,
    lower: Option<f64>,
    dir: i8,
    value: Option<f64>,
}

impl SuperTrend {
    pub fn new(period: usize, mult: f64) -> Self {
        SuperTrend { atr: Rma::new(period), mult, prev_close: None, upper: None, lower: None, dir: 1, value: None }
    }
}

impl Indicator for SuperTrend {
    fn update(&mut self, c: &Candle) {
        let tr = true_range(c.high, c.low, self.prev_close);
        let prev_close = self.prev_close;
        self.prev_close = Some(c.close);
        let Some(atr) = self.atr.update(tr) else { return };
        let basic_up = c.hl2() + self.mult * atr;
        let basic_lo = c.hl2() - self.mult * atr;
        let upper = match (self.upper, prev_close) {
            (Some(u), Some(pc)) if basic_up > u && pc <= u => u,
            _ => basic_up,
        };
        let lower = match (self.lower, prev_close) {
            (Some(l), Some(pc)) if basic_lo < l && pc >= l => l,
            _ => basic_lo,
        };
        if self.value.is_some() {
            if self.dir == 1 && c.close < lower {
                self.dir = -1;
            } else if self.dir == -1 && c.close > upper {
                self.dir = 1;
            }
        } else {
            self.dir = if c.close >= c.hl2() { 1 } else { -1 };
        }
        self.upper = Some(upper);
        self.lower = Some(lower);
        self.value = Some(if self.dir == 1 { lower } else { upper });
    }

    fn get(&self, i: usize) -> Option<f64> {
        match i {
            0 => self.value,
            _ => self.value.map(|_| self.dir as f64),
        }
    }
}

/// ADX with +DI and -DI (Wilder).
pub struct Adx {
    prev: Option<Candle>,
    tr: Rma,
    plus: Rma,
    minus: Rma,
    adx: Rma,
    di: Option<(f64, f64)>,
}

impl Adx {
    pub fn new(period: usize) -> Self {
        Adx {
            prev: None,
            tr: Rma::new(period),
            plus: Rma::new(period),
            minus: Rma::new(period),
            adx: Rma::new(period),
            di: None,
        }
    }
}

impl Indicator for Adx {
    fn update(&mut self, c: &Candle) {
        let Some(p) = self.prev.replace(*c) else { return };
        let up = c.high - p.high;
        let down = p.low - c.low;
        let plus_dm = if up > down && up > 0.0 { up } else { 0.0 };
        let minus_dm = if down > up && down > 0.0 { down } else { 0.0 };
        let tr = true_range(c.high, c.low, Some(p.close));
        let (Some(tr), Some(pdm), Some(mdm)) =
            (self.tr.update(tr), self.plus.update(plus_dm), self.minus.update(minus_dm))
        else {
            return;
        };
        if tr <= 0.0 {
            return;
        }
        let (pdi, mdi) = (100.0 * pdm / tr, 100.0 * mdm / tr);
        self.di = Some((pdi, mdi));
        let dx = if pdi + mdi > 0.0 { 100.0 * (pdi - mdi).abs() / (pdi + mdi) } else { 0.0 };
        self.adx.update(dx);
    }

    fn get(&self, i: usize) -> Option<f64> {
        match i {
            0 => self.adx.value(),
            1 => self.di.map(|d| d.0),
            _ => self.di.map(|d| d.1),
        }
    }
}
