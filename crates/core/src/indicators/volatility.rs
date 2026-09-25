use super::Indicator;
use super::primitives::{Ema, MinMax, Rma, Sma, true_range};
use crate::candle::Candle;

pub struct Bbands {
    sma: Sma,
    k: f64,
}

impl Bbands {
    pub fn new(period: usize, k: f64) -> Self {
        Bbands { sma: Sma::new(period), k }
    }
}

impl Indicator for Bbands {
    fn update(&mut self, c: &Candle) {
        self.sma.update(c.close);
    }

    fn get(&self, i: usize) -> Option<f64> {
        let (m, sd) = (self.sma.value()?, self.sma.stddev()?);
        Some(match i {
            0 => m,
            1 => m + self.k * sd,
            2 => m - self.k * sd,
            // Band width as a percentage of the middle band.
            _ => {
                if m != 0.0 {
                    100.0 * 2.0 * self.k * sd / m
                } else {
                    0.0
                }
            }
        })
    }
}

pub struct Atr {
    prev_close: Option<f64>,
    rma: Rma,
}

impl Atr {
    pub fn new(period: usize) -> Self {
        Atr { prev_close: None, rma: Rma::new(period) }
    }

    pub fn update_bar(&mut self, c: &Candle) -> Option<f64> {
        let tr = true_range(c.high, c.low, self.prev_close.replace(c.close));
        self.rma.update(tr)
    }
}

impl Indicator for Atr {
    fn update(&mut self, c: &Candle) {
        self.update_bar(c);
    }

    fn get(&self, _: usize) -> Option<f64> {
        self.rma.value()
    }
}

pub struct Donchian(MinMax);

impl Donchian {
    pub fn new(period: usize) -> Self {
        Donchian(MinMax::new(period))
    }
}

impl Indicator for Donchian {
    fn update(&mut self, c: &Candle) {
        self.0.update(c.high, c.low);
    }

    fn get(&self, i: usize) -> Option<f64> {
        let (hi, lo) = self.0.value()?;
        Some(match i {
            0 => hi,
            1 => lo,
            _ => (hi + lo) / 2.0,
        })
    }
}

pub struct Keltner {
    ema: Ema,
    atr: Atr,
    mult: f64,
}

impl Keltner {
    pub fn new(period: usize, mult: f64, atr_period: usize) -> Self {
        Keltner { ema: Ema::new(period), atr: Atr::new(atr_period), mult }
    }
}

impl Indicator for Keltner {
    fn update(&mut self, c: &Candle) {
        self.ema.update(c.close);
        self.atr.update_bar(c);
    }

    fn get(&self, i: usize) -> Option<f64> {
        let (m, a) = (self.ema.value()?, self.atr.get(0)?);
        Some(match i {
            0 => m,
            1 => m + self.mult * a,
            _ => m - self.mult * a,
        })
    }
}
