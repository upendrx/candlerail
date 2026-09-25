//! Built-in strategy templates, compiled into the binary.

pub const ALL: &[(&str, &str)] = &[
    ("ema-crossover", include_str!("../../../templates/ema-crossover.json")),
    ("rsi-dip-uptrend", include_str!("../../../templates/rsi-dip-uptrend.json")),
    ("bollinger-reversion", include_str!("../../../templates/bollinger-reversion.json")),
    ("donchian-breakout", include_str!("../../../templates/donchian-breakout.json")),
    ("macd-momentum", include_str!("../../../templates/macd-momentum.json")),
    ("supertrend-long-short", include_str!("../../../templates/supertrend-long-short.json")),
    ("adx-trend-strength", include_str!("../../../templates/adx-trend-strength.json")),
    ("vwap-intraday", include_str!("../../../templates/vwap-intraday.json")),
    ("stochastic-oversold", include_str!("../../../templates/stochastic-oversold.json")),
    ("keltner-breakout", include_str!("../../../templates/keltner-breakout.json")),
    ("pa-pin-bar-support", include_str!("../../../templates/pa-pin-bar-support.json")),
    ("pa-inside-bar-breakout", include_str!("../../../templates/pa-inside-bar-breakout.json")),
    ("pa-engulfing-structure", include_str!("../../../templates/pa-engulfing-structure.json")),
    ("pa-previous-day-breakout", include_str!("../../../templates/pa-previous-day-breakout.json")),
    ("pa-opening-range-breakout", include_str!("../../../templates/pa-opening-range-breakout.json")),
    ("pa-scalp-micro-breakout", include_str!("../../../templates/pa-scalp-micro-breakout.json")),
    ("pa-monthly-level-breakout", include_str!("../../../templates/pa-monthly-level-breakout.json")),
    ("pa-morning-star-support", include_str!("../../../templates/pa-morning-star-support.json")),
];

pub fn get(id: &str) -> Option<&'static str> {
    ALL.iter().find(|(k, _)| *k == id).map(|(_, v)| *v)
}

#[cfg(test)]
mod tests {
    use candlerail_core::Strategy;

    #[test]
    fn every_template_is_valid() {
        for (id, text) in super::ALL {
            let s = Strategy::from_json(text).unwrap_or_else(|e| panic!("{id}: {e}"));
            assert!(s.validate().is_empty(), "{id}: {:?}", s.validate());
            assert!(s.about.is_some(), "{id} needs an about section");
            assert!(s.market.interval.is_some(), "{id} needs a default interval");
        }
    }
}
