//! End-to-end tests of the `candlerail` binary. They use the bundled sample CSV,
//! so they run offline.

use std::path::PathBuf;
use std::process::{Command, Output};

fn bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_candlerail"))
}

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn run(args: &[&str]) -> Output {
    let out = bin().args(args).current_dir(root()).output().expect("run candlerail");
    assert!(
        out.status.success(),
        "candlerail {args:?} failed:\n{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    out
}

fn stdout(o: &Output) -> String {
    String::from_utf8_lossy(&o.stdout).into_owned()
}

#[test]
fn lists_templates_and_indicators() {
    let t = stdout(&run(&["templates"]));
    assert!(t.contains("ema-crossover") && t.contains("vwap-intraday"));
    let i = stdout(&run(&["indicators"]));
    for kind in ["sma", "rsi", "macd", "supertrend", "keltner"] {
        assert!(i.contains(kind), "missing {kind}");
    }
}

#[test]
fn explains_every_template() {
    let list = stdout(&run(&["templates"]));
    let ids: Vec<&str> =
        list.lines().take_while(|l| !l.trim().is_empty()).filter_map(|l| l.split_whitespace().next()).collect();
    assert_eq!(ids.len(), 10);
    for id in ids {
        let text = stdout(&run(&["explain", id]));
        assert!(text.contains("No problems found."), "{id}:\n{text}");
    }
}

#[test]
fn backtests_the_sample_csv_and_writes_json() {
    let report = std::env::temp_dir().join(format!("candlerail-test-{}.json", std::process::id()));
    let out = stdout(&run(&[
        "backtest",
        "donchian-breakout",
        "--csv",
        "examples/btcusdt-1h-sample.csv",
        "--interval",
        "1h",
        "--json",
        report.to_str().unwrap(),
    ]));
    assert!(out.contains("Return") && out.contains("Max drawdown"));
    let json: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&report).unwrap()).unwrap();
    assert_eq!(json["bars"], 2208);
    assert!(json["equity"].as_array().unwrap().len() == 2208);
    let _ = std::fs::remove_file(report);
}

#[test]
fn creates_a_strategy_from_a_template() {
    let file = std::env::temp_dir().join(format!("candlerail-new-{}.json", std::process::id()));
    let _ = std::fs::remove_file(&file);
    run(&["new", file.to_str().unwrap(), "--from", "macd-momentum"]);
    let check = stdout(&run(&["check", file.to_str().unwrap()]));
    assert!(check.ends_with("ok\n"), "{check}");
    let again = bin().args(["new", file.to_str().unwrap()]).output().unwrap();
    assert!(!again.status.success(), "refuses to overwrite");
    let _ = std::fs::remove_file(file);
}

#[test]
fn reports_invalid_strategies_clearly() {
    let file = std::env::temp_dir().join(format!("candlerail-bad-{}.json", std::process::id()));
    std::fs::write(&file, r#"{ "name": "bad", "entry": { "long": { "left": "rsi", "op": ">", "right": 70 } } }"#)
        .unwrap();
    let out = bin().args(["check", file.to_str().unwrap()]).output().unwrap();
    assert!(!out.status.success());
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(err.contains("unknown value `rsi`") && err.contains("exit:"), "{err}");
    let _ = std::fs::remove_file(file);
}

#[test]
fn prompt_covers_every_indicator() {
    let p = stdout(&run(&["prompt"]));
    for kind in ["\"sma\"", "\"vwap\"", "\"donchian\"", "risk_multiple", "crosses_above"] {
        assert!(p.contains(kind), "prompt is missing {kind}");
    }
}
