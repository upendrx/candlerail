//! The local web app: static UI plus a small JSON API.
//!
//! | Route | |
//! |---|---|
//! | `GET /` | the app |
//! | `GET /api/catalog` | indicators, price fields, operators, intervals |
//! | `GET /api/templates` | built-in strategies |
//! | `POST /api/check` | validate a strategy and explain it in plain English |
//! | `POST /api/backtest` | run a backtest; returns the report and the candles |
//! | `GET /api/prompt` | instructions for an AI assistant |
//! | `GET /schema.json` | JSON Schema for strategy files |

use crate::{default_span, now_ms, prompt, templates};
use axum::extract::State;
use axum::http::{StatusCode, header};
use axum::response::{Html, IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use candlerail_core::indicators::CATALOG;
use candlerail_core::spec::PRICE_FIELDS;
use candlerail_core::{BacktestConfig, Interval, Strategy, explain, time};
use candlerail_data::{Query, binance};
use serde::Deserialize;
use serde_json::{Value, json};
use std::path::PathBuf;
use std::sync::Arc;

const INDEX: &str = include_str!("../../../ui/index.html");
const CHARTS: &str = include_str!("../../../ui/vendor/lightweight-charts.js");
const SCHEMA: &str = include_str!("../../../schema/strategy.schema.json");

struct App {
    cache: PathBuf,
    api: String,
    ui_dir: Option<PathBuf>,
}

pub fn serve(listen: &str, ui_dir: Option<PathBuf>, cache: PathBuf, api: String) -> anyhow::Result<()> {
    let app = Arc::new(App { cache, api, ui_dir });
    let router = Router::new()
        .route("/", get(index))
        .route("/vendor/lightweight-charts.js", get(charts))
        .route("/schema.json", get(|| async { ([(header::CONTENT_TYPE, "application/json")], SCHEMA) }))
        .route("/api/catalog", get(catalog))
        .route("/api/templates", get(list_templates))
        .route(
            "/api/prompt",
            get(|| async { ([(header::CONTENT_TYPE, "text/plain; charset=utf-8")], prompt::build()) }),
        )
        .route("/api/check", post(check))
        .route("/api/backtest", post(backtest))
        .with_state(app);
    let rt = tokio::runtime::Builder::new_multi_thread().worker_threads(2).enable_all().build()?;
    rt.block_on(async {
        let listener = tokio::net::TcpListener::bind(listen).await?;
        println!("candlerail is running at http://{listen}  (Ctrl-C to stop)");
        axum::serve(listener, router)
            .with_graceful_shutdown(async {
                let _ = tokio::signal::ctrl_c().await;
            })
            .await?;
        Ok(())
    })
}

async fn index(State(app): State<Arc<App>>) -> Html<String> {
    if let Some(dir) = &app.ui_dir
        && let Ok(s) = std::fs::read_to_string(dir.join("index.html"))
    {
        return Html(s);
    }
    Html(INDEX.to_string())
}

async fn charts() -> impl IntoResponse {
    ([(header::CONTENT_TYPE, "application/javascript"), (header::CACHE_CONTROL, "max-age=86400")], CHARTS)
}

async fn catalog() -> Json<Value> {
    Json(json!({
        "indicators": CATALOG,
        "price_fields": PRICE_FIELDS,
        "ops": [">", "<", ">=", "<=", "crosses_above", "crosses_below", "rising", "falling"],
        "intervals": Interval::ALL.iter().map(|i| i.as_str()).collect::<Vec<_>>(),
    }))
}

async fn list_templates() -> Json<Value> {
    let list: Vec<Value> = templates::ALL
        .iter()
        .filter_map(|(id, text)| {
            serde_json::from_str::<Value>(text).ok().map(|spec| json!({ "id": id, "strategy": spec }))
        })
        .collect();
    Json(Value::Array(list))
}

/// A strategy arrives either as JSON or as pasted text (e.g. an AI reply).
fn parse_strategy(v: &Value) -> Result<Strategy, String> {
    match v {
        Value::String(s) => Strategy::from_ai_text(s),
        other => serde_json::from_value(other.clone()).map_err(|e| format!("not a valid strategy: {e}")),
    }
}

#[derive(Deserialize)]
struct CheckReq {
    strategy: Value,
}

async fn check(Json(req): Json<CheckReq>) -> Json<Value> {
    match parse_strategy(&req.strategy) {
        Ok(s) => {
            Json(json!({ "ok": true, "errors": s.validate(), "explanation": explain::explain(&s), "strategy": s }))
        }
        Err(e) => Json(json!({ "ok": false, "errors": [e] })),
    }
}

#[derive(Deserialize)]
struct BacktestReq {
    strategy: Value,
    symbol: Option<String>,
    interval: Option<String>,
    from: Option<String>,
    to: Option<String>,
    capital: Option<f64>,
    /// Candles as CSV text, instead of downloading.
    csv: Option<String>,
}

fn err(status: StatusCode, msgs: Vec<String>) -> Response {
    (status, Json(json!({ "ok": false, "errors": msgs }))).into_response()
}

async fn backtest(State(app): State<Arc<App>>, Json(req): Json<BacktestReq>) -> Response {
    let s = match parse_strategy(&req.strategy) {
        Ok(s) => s,
        Err(e) => return err(StatusCode::BAD_REQUEST, vec![e]),
    };
    let problems = s.validate();
    if !problems.is_empty() {
        return err(StatusCode::BAD_REQUEST, problems);
    }
    let interval = match req.interval.as_deref().filter(|x| !x.is_empty()) {
        Some(i) => match i.parse::<Interval>() {
            Ok(i) => i,
            Err(e) => return err(StatusCode::BAD_REQUEST, vec![e]),
        },
        None => s.market.interval.unwrap_or(Interval::H1),
    };
    let symbol = req
        .symbol
        .filter(|x| !x.trim().is_empty())
        .or_else(|| s.market.symbol.clone())
        .unwrap_or_else(|| "BTCUSDT".into())
        .trim()
        .to_uppercase();
    let date = |v: &Option<String>| v.as_deref().filter(|x| !x.is_empty()).map(time::parse);
    let to = match date(&req.to) {
        Some(Some(t)) => t,
        Some(None) => return err(StatusCode::BAD_REQUEST, vec!["bad end date".into()]),
        None => now_ms(),
    };
    let from = match date(&req.from) {
        Some(Some(t)) => t,
        Some(None) => return err(StatusCode::BAD_REQUEST, vec!["bad start date".into()]),
        None => to - default_span(interval),
    };
    let capital = req.capital.filter(|c| *c > 0.0).unwrap_or(10_000.0);
    let csv = req.csv;
    let app2 = app.clone();
    let loaded = tokio::task::spawn_blocking(move || -> anyhow::Result<(String, Vec<candlerail_core::Candle>)> {
        if let Some(text) = csv.filter(|t| !t.trim().is_empty()) {
            let c = candlerail_data::csv::parse(&text)?;
            return Ok(("CSV".into(), candlerail_data::tidy(c, from, to)));
        }
        let q = Query { symbol: symbol.clone(), interval, from, to };
        Ok((symbol, binance::load(&app2.api, &app2.cache, &q, |_| {})?))
    })
    .await;
    let (symbol, candles) = match loaded {
        Ok(Ok(v)) => v,
        Ok(Err(e)) => return err(StatusCode::BAD_GATEWAY, vec![format!("{e:#}")]),
        Err(e) => return err(StatusCode::INTERNAL_SERVER_ERROR, vec![e.to_string()]),
    };
    if candles.len() < 50 {
        return err(
            StatusCode::BAD_REQUEST,
            vec![format!("only {} candles in that range; pick a longer period", candles.len())],
        );
    }
    let cfg = BacktestConfig { capital, interval, ..BacktestConfig::default() };
    let (report, candles) =
        match tokio::task::spawn_blocking(move || (candlerail_core::run(&s, &candles, &cfg), candles)).await {
            Ok((Ok(r), c)) => (r, c),
            Ok((Err(e), _)) => return err(StatusCode::BAD_REQUEST, e),
            Err(e) => return err(StatusCode::INTERNAL_SERVER_ERROR, vec![e.to_string()]),
        };
    Json(json!({ "ok": true, "symbol": symbol, "interval": interval, "report": report, "candles": candles }))
        .into_response()
}
