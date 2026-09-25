use super::*;

fn bar(i: i64, o: f64, h: f64, l: f64, c: f64, v: f64) -> Candle {
    Candle { ts: i * 3_600_000, open: o, high: h, low: l, close: c, volume: v }
}

fn closes(xs: &[f64]) -> Vec<Candle> {
    xs.iter().enumerate().map(|(i, &c)| bar(i as i64, c, c + 1.0, c - 1.0, c, 10.0)).collect()
}

fn run(kind: &str, given: serde_json::Value, bars: &[Candle]) -> Box<dyn Indicator> {
    let inf = info(kind).unwrap();
    let ps = resolve_params(inf, given.as_object().unwrap()).unwrap();
    let mut ind = build(kind, &ps).unwrap();
    for b in bars {
        ind.update(b);
    }
    ind
}

fn close_to(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-9
}

#[test]
fn every_catalog_entry_builds() {
    for inf in CATALOG {
        let ps = resolve_params(inf, &Map::new()).unwrap();
        let mut ind = build(inf.kind, &ps).unwrap();
        for b in closes(&(0..300).map(|i| 100.0 + (i as f64 * 0.3).sin() * 5.0 + i as f64 * 0.1).collect::<Vec<_>>()) {
            ind.update(&b);
        }
        for (i, out) in inf.outputs.iter().enumerate() {
            let v = ind.get(i);
            assert!(v.is_some_and(f64::is_finite), "{}.{out} not ready after 300 bars: {v:?}", inf.kind);
        }
    }
}

#[test]
fn sma_and_ema() {
    let b = closes(&[1.0, 2.0, 3.0, 4.0, 5.0]);
    assert!(close_to(run("sma", serde_json::json!({"period": 3}), &b).get(0).unwrap(), 4.0));
    // EMA(3): seeded with SMA(1,2,3) = 2, then alpha 0.5: 3, then 4.
    assert!(close_to(run("ema", serde_json::json!({"period": 3}), &b).get(0).unwrap(), 4.0));
    assert!(run("sma", serde_json::json!({"period": 6}), &b).get(0).is_none(), "warm-up");
}

#[test]
fn rsi_bounds() {
    let up = closes(&(0..30).map(|i| 100.0 + i as f64).collect::<Vec<_>>());
    assert!(close_to(run("rsi", serde_json::json!({}), &up).get(0).unwrap(), 100.0));
    let zigzag = closes(&(0..40).map(|i| if i % 2 == 0 { 100.0 } else { 101.0 }).collect::<Vec<_>>());
    let v = run("rsi", serde_json::json!({}), &zigzag).get(0).unwrap();
    assert!((v - 50.0).abs() < 5.0, "balanced gains/losses near 50, got {v}");
}

#[test]
fn bands_are_symmetric_around_the_average() {
    let b = closes(&(0..50).map(|i| 100.0 + (i % 7) as f64).collect::<Vec<_>>());
    let bb = run("bbands", serde_json::json!({"period": 20, "stddev": 2}), &b);
    let sma = run("sma", serde_json::json!({"period": 20}), &b);
    let (m, u, l) = (bb.get(0).unwrap(), bb.get(1).unwrap(), bb.get(2).unwrap());
    assert!(close_to(m, sma.get(0).unwrap()));
    assert!(close_to(u - m, m - l) && u > m);
    let k = run("keltner", serde_json::json!({}), &b);
    assert!(close_to(k.get(0).unwrap(), run("ema", serde_json::json!({"period": 20}), &b).get(0).unwrap()));
}

#[test]
fn atr_of_constant_range_is_the_range() {
    let b: Vec<Candle> = (0..30).map(|i| bar(i, 100.0, 102.0, 98.0, 100.0, 1.0)).collect();
    assert!(close_to(run("atr", serde_json::json!({"period": 5}), &b).get(0).unwrap(), 4.0));
}

#[test]
fn channels_and_stochastic() {
    let b = closes(&[10.0, 12.0, 11.0, 15.0, 13.0]);
    let d = run("donchian", serde_json::json!({"period": 3}), &b);
    assert_eq!((d.get(0).unwrap(), d.get(1).unwrap()), (16.0, 10.0)); // highs are close+1, lows close-1
    let top: Vec<Candle> =
        (0..20).map(|i| bar(i, 100.0 + i as f64, 101.0 + i as f64, 99.0 + i as f64, 101.0 + i as f64, 1.0)).collect();
    let s = run("stoch", serde_json::json!({}), &top);
    assert!(s.get(0).unwrap() > 90.0, "closing at the highs keeps %K near 100");
}

#[test]
fn vwap_resets_each_day() {
    let day = 86_400_000;
    let b = [
        Candle { ts: 0, open: 1.0, high: 1.0, low: 1.0, close: 1.0, volume: 1.0 },
        Candle { ts: 3_600_000, open: 3.0, high: 3.0, low: 3.0, close: 3.0, volume: 3.0 },
        Candle { ts: day, open: 10.0, high: 10.0, low: 10.0, close: 10.0, volume: 5.0 },
    ];
    let mut v = run("vwap", serde_json::json!({}), &b[..2]);
    assert!(close_to(v.get(0).unwrap(), (1.0 + 9.0) / 4.0));
    v.update(&b[2]);
    assert!(close_to(v.get(0).unwrap(), 10.0), "new day starts fresh");
}

#[test]
fn trend_indicators_agree_with_the_trend() {
    let mut xs: Vec<f64> = (0..80).map(|i| 100.0 + i as f64).collect();
    xs.extend((0..80).map(|i| 180.0 - 2.0 * i as f64));
    let b = closes(&xs);
    let st = run("supertrend", serde_json::json!({}), &b[..80]);
    assert_eq!(st.get(1), Some(1.0));
    let st = run("supertrend", serde_json::json!({}), &b);
    assert_eq!(st.get(1), Some(-1.0), "flips after the reversal");
    let adx = run("adx", serde_json::json!({}), &b[..80]);
    assert!(adx.get(0).unwrap() > 25.0 && adx.get(1).unwrap() > adx.get(2).unwrap());
    let m = run("macd", serde_json::json!({}), &closes(&[50.0; 60]));
    assert!(close_to(m.get(0).unwrap(), 0.0) && close_to(m.get(2).unwrap(), 0.0));
}

#[test]
fn obv_and_roc() {
    let b = [bar(0, 1.0, 1.0, 1.0, 10.0, 5.0), bar(1, 1.0, 1.0, 1.0, 11.0, 3.0), bar(2, 1.0, 1.0, 1.0, 10.5, 2.0)];
    assert!(close_to(run("obv", serde_json::json!({}), &b).get(0).unwrap(), 1.0));
    assert!(close_to(run("roc", serde_json::json!({"period": 1}), &b[..2]).get(0).unwrap(), 10.0));
}

#[test]
fn parameter_validation() {
    let rsi = info("rsi").unwrap();
    assert!(resolve_params(rsi, serde_json::json!({"period": 1}).as_object().unwrap()).is_err());
    assert!(resolve_params(rsi, serde_json::json!({"period": 14.5}).as_object().unwrap()).is_err());
    let e = resolve_params(rsi, serde_json::json!({"lenght": 14}).as_object().unwrap()).unwrap_err();
    assert!(e.contains("lenght") && e.contains("period"), "{e}");
    assert_eq!(label(rsi, &[14.0]), "RSI(14)");
    assert_eq!(label(info("bbands").unwrap(), &[20.0, 2.5]), "BBANDS(20, 2.5)");
}

fn ohlc(i: i64, o: f64, h: f64, l: f64, c: f64) -> Candle {
    bar(i, o, h, l, c, 10.0)
}

fn out(kind: &str, name: &str) -> usize {
    info(kind).unwrap().outputs.iter().position(|o| *o == name).unwrap()
}

#[test]
fn candle_patterns() {
    let p =
        |bars: &[Candle], name: &str| run("patterns", serde_json::json!({}), bars).get(out("patterns", name)).unwrap();
    let down = ohlc(0, 10.0, 10.2, 8.8, 9.0);
    let engulf = ohlc(1, 8.9, 10.5, 8.8, 10.4);
    assert_eq!(p(&[down, engulf], "bullish_engulfing"), 1.0);
    assert_eq!(p(&[down, engulf], "bearish_engulfing"), 0.0);
    assert_eq!(p(&[ohlc(0, 10.0, 10.1, 7.0, 10.05)], "hammer"), 1.0);
    assert_eq!(p(&[ohlc(0, 10.0, 13.0, 9.95, 9.98)], "shooting_star"), 1.0);
    assert_eq!(p(&[ohlc(0, 10.0, 11.0, 9.0, 10.02)], "doji"), 1.0);
    let mother = ohlc(0, 10.0, 12.0, 8.0, 11.0);
    assert_eq!(p(&[mother, ohlc(1, 10.5, 11.5, 9.0, 10.0)], "inside_bar"), 1.0);
    assert_eq!(p(&[mother, ohlc(1, 10.5, 12.5, 7.5, 10.0)], "outside_bar"), 1.0);
    let star = [ohlc(0, 12.0, 12.1, 9.9, 10.0), ohlc(1, 9.8, 10.0, 9.4, 9.7), ohlc(2, 9.8, 11.6, 9.7, 11.5)];
    assert_eq!(p(&star, "morning_star"), 1.0);
    let soldiers = [ohlc(0, 10.0, 11.1, 9.9, 11.0), ohlc(1, 10.8, 12.1, 10.7, 12.0), ohlc(2, 11.8, 13.1, 11.7, 13.0)];
    assert_eq!(p(&soldiers, "three_white_soldiers"), 1.0);
    assert_eq!(p(&[ohlc(0, 10.0, 12.0, 10.0, 12.0)], "bullish_marubozu"), 1.0);
}

#[test]
fn swings_confirm_without_looking_ahead() {
    // Highs rise to a peak at bar 3, then fall.
    let highs = [10.0, 11.0, 12.0, 15.0, 13.0, 12.0, 11.0];
    let bars: Vec<Candle> =
        highs.iter().enumerate().map(|(i, &h)| ohlc(i as i64, h - 1.0, h, h - 2.0, h - 0.5)).collect();
    let args = serde_json::json!({"left": 2, "right": 2});
    assert!(run("swings", args.clone(), &bars[..5]).get(0).is_none(), "not yet confirmed one bar after the peak");
    assert_eq!(run("swings", args, &bars[..6]).get(0), Some(15.0), "confirmed two bars later");
}

#[test]
fn higher_timeframe_levels() {
    let h = 3_600_000;
    let day = 24 * h;
    let bars = [
        Candle { ts: 0, open: 100.0, high: 105.0, low: 99.0, close: 104.0, volume: 1.0 },
        Candle { ts: 12 * h, open: 104.0, high: 110.0, low: 95.0, close: 108.0, volume: 1.0 },
        Candle { ts: day, open: 108.0, high: 109.0, low: 107.0, close: 108.5, volume: 1.0 },
    ];
    let p = run("period", serde_json::json!({"minutes": 1440}), &bars);
    let g = |name: &str| p.get(out("period", name)).unwrap();
    assert_eq!((g("prev_high"), g("prev_low"), g("prev_close"), g("prev_open")), (110.0, 95.0, 108.0, 100.0));
    assert_eq!((g("open"), g("high")), (108.0, 109.0));
}

#[test]
fn opening_range_and_relative_volume() {
    let m = 60_000;
    let bars: Vec<Candle> = (0..30)
        .map(|i| Candle { ts: i * 5 * m, open: 10.0, high: 10.0 + (i % 4) as f64, low: 9.0, close: 10.0, volume: 1.0 })
        .collect();
    let or = run("opening_range", serde_json::json!({"minutes": 15}), &bars[..3]);
    assert_eq!((or.get(0), or.get(2)), (Some(12.0), Some(0.0)), "first three 5m bars form the range");
    let or = run("opening_range", serde_json::json!({"minutes": 15}), &bars[..4]);
    assert_eq!(or.get(2), Some(1.0));
    let mut v: Vec<Candle> = (0..21).map(|i| bar(i, 1.0, 1.0, 1.0, 1.0, 100.0)).collect();
    v.push(bar(21, 1.0, 1.0, 1.0, 1.0, 300.0));
    let va = run("volume_avg", serde_json::json!({"period": 20}), &v);
    assert!((va.get(1).unwrap() - 3.0).abs() < 1e-9, "a 300 bar after twenty 100 bars is 3x");
}
