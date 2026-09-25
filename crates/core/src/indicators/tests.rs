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
