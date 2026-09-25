use super::Indicator;
use crate::candle::Candle;
use crate::time;

/// Session VWAP, reset at 00:00 UTC each day.
#[derive(Default)]
pub struct Vwap {
    day: Option<i64>,
    pv: f64,
    vol: f64,
}

impl Indicator for Vwap {
    fn update(&mut self, c: &Candle) {
        let d = time::day(c.ts);
        if self.day != Some(d) {
            self.day = Some(d);
            self.pv = 0.0;
            self.vol = 0.0;
        }
        self.pv += c.hlc3() * c.volume;
        self.vol += c.volume;
    }

    fn get(&self, _: usize) -> Option<f64> {
        (self.vol > 0.0).then(|| self.pv / self.vol)
    }
}

#[derive(Default)]
pub struct Obv {
    prev: Option<f64>,
    total: f64,
    started: bool,
}

impl Indicator for Obv {
    fn update(&mut self, c: &Candle) {
        if let Some(p) = self.prev {
            if c.close > p {
                self.total += c.volume;
            } else if c.close < p {
                self.total -= c.volume;
            }
        }
        self.prev = Some(c.close);
        self.started = true;
    }

    fn get(&self, _: usize) -> Option<f64> {
        self.started.then_some(self.total)
    }
}
