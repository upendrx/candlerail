//! Parameter studies: expand strategy templates over a grid of settings, run
//! every variant on several markets and timeframes, and judge the ones picked
//! on the first part of the period by how they did on the rest.
//!
//! A template is an ordinary strategy file in which values can be
//! placeholders:
//!
//! * `"{{rr}}"` is replaced by the grid value of `rr` (a number, a string or a
//!   whole object);
//! * `"{{stop.v}}"` reads the field `v` of an object value;
//! * `"{{hours:3}}"` becomes the number of candles in three hours at the
//!   interval being tested;
//! * placeholders inside longer strings, such as `"sw.support + {{k}} * atr"`,
//!   are replaced by their text.
//!
//! Selection is decided before anything is looked at out of sample: a variant
//! is *selected* when it trades often enough and did well in sample on enough
//! markets, and it *survived* if it then stayed profitable on the held-out part.

use crate::backtest::{self, BacktestConfig};
use crate::broker::Trade;
use crate::candle::{Candle, Interval};
use crate::spec::Strategy;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Study {
    pub name: String,
    #[serde(default)]
    pub description: String,
    pub symbols: Vec<String>,
    pub intervals: Vec<Interval>,
    /// Days of history to test.
    pub days: u32,
    /// End the period this many days ago instead of now, to test on older data.
    #[serde(default)]
    pub offset_days: u32,
    /// Share of the period used to select variants; the rest is held out.
    #[serde(default = "default_split")]
    pub split: f64,
    #[serde(default = "default_capital")]
    pub capital: f64,
    /// Trading-cost scenarios; every variant runs under each.
    pub costs: Vec<CostCase>,
    #[serde(default)]
    pub selection: Selection,
    /// Grid shared by every family; a family's own grid adds to or replaces it.
    #[serde(default)]
    pub grid: BTreeMap<String, Vec<Value>>,
    pub families: Vec<Family>,
}

fn default_split() -> f64 {
    0.67
}

fn default_capital() -> f64 {
    10_000.0
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CostCase {
    pub label: String,
    pub fee_bps: f64,
    pub slippage_bps: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct Selection {
    /// Count trades per day across all markets together (a portfolio traded
    /// side by side) instead of per market.
    pub portfolio: bool,
    pub min_trades_per_day: f64,
    pub max_trades_per_day: f64,
    /// Minimum pooled average R in sample.
    pub min_avg_r: f64,
    /// Markets that must be profitable, in sample to be selected and out of
    /// sample to survive.
    pub min_markets: usize,
}

impl Default for Selection {
    fn default() -> Self {
        Selection {
            portfolio: false,
            min_trades_per_day: 1.0,
            max_trades_per_day: 12.0,
            min_avg_r: 0.05,
            min_markets: 3,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Family {
    pub id: String,
    pub name: String,
    /// What the setup is looking for, in a sentence or two.
    pub idea: String,
    #[serde(default)]
    pub kind: String,
    #[serde(default)]
    pub grid: BTreeMap<String, Vec<Value>>,
    /// The strategy template.
    pub strategy: Value,
}

/// One combination of grid values.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Variant {
    pub index: usize,
    /// Display label of each setting.
    pub params: BTreeMap<String, String>,
    #[serde(skip)]
    pub values: BTreeMap<String, Value>,
}

fn label_of(v: &Value) -> String {
    match v {
        Value::Object(m) => m.get("label").and_then(Value::as_str).map(str::to_string).unwrap_or_else(|| v.to_string()),
        Value::String(s) => s.clone(),
        other => other.to_string(),
    }
}

/// Every combination of the study's and the family's grid, in a stable order.
pub fn variants(study: &Study, family: &Family) -> Vec<Variant> {
    let mut grid = study.grid.clone();
    grid.extend(family.grid.clone());
    let used = family.strategy.to_string();
    // Only settings the template refers to multiply the variants.
    grid.retain(|k, _| used.contains(&format!("{{{{{k}}}")) || used.contains(&format!("{{{{{k}.")));
    let mut out = vec![BTreeMap::new()];
    for (k, vals) in &grid {
        out = out
            .into_iter()
            .flat_map(|base: BTreeMap<String, Value>| {
                vals.iter().map(move |v| {
                    let mut m = base.clone();
                    m.insert(k.clone(), v.clone());
                    m
                })
            })
            .collect();
    }
    out.into_iter()
        .enumerate()
        .map(|(index, values)| Variant {
            index,
            params: values.iter().map(|(k, v)| (k.clone(), label_of(v))).collect(),
            values,
        })
        .collect()
}

fn resolve(expr: &str, vars: &BTreeMap<String, Value>, interval: Interval) -> Result<Value, String> {
    let expr = expr.trim();
    if let Some(h) = expr.strip_prefix("hours:") {
        let hours: f64 = h.trim().parse().map_err(|_| format!("bad placeholder `{{{{{expr}}}}}`"))?;
        let bars = (hours * 3_600_000.0 / interval.millis() as f64).round().max(1.0);
        return Ok(Value::from(bars as u64));
    }
    let mut path = expr.split('.');
    let head = path.next().unwrap_or_default();
    let mut v = vars.get(head).ok_or_else(|| format!("unknown setting `{head}` in `{{{{{expr}}}}}`"))?;
    for p in path {
        v = v.get(p).ok_or_else(|| format!("setting `{head}` has no field `{p}`"))?;
    }
    Ok(v.clone())
}

fn text(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        other => other.to_string(),
    }
}

fn substitute(v: &Value, vars: &BTreeMap<String, Value>, interval: Interval) -> Result<Value, String> {
    Ok(match v {
        Value::String(s) => {
            let t = s.trim();
            if t.starts_with("{{") && t.ends_with("}}") && t.matches("{{").count() == 1 {
                resolve(&t[2..t.len() - 2], vars, interval)?
            } else if s.contains("{{") {
                let mut out = String::new();
                let mut rest = s.as_str();
                while let Some(a) = rest.find("{{") {
                    let b = rest[a..].find("}}").ok_or_else(|| format!("unclosed placeholder in `{s}`"))? + a;
                    out.push_str(&rest[..a]);
                    out.push_str(&text(&resolve(&rest[a + 2..b], vars, interval)?));
                    rest = &rest[b + 2..];
                }
                out.push_str(rest);
                Value::String(out)
            } else {
                v.clone()
            }
        }
        Value::Array(a) => Value::Array(a.iter().map(|x| substitute(x, vars, interval)).collect::<Result<_, _>>()?),
        Value::Object(m) => Value::Object(
            m.iter().map(|(k, x)| Ok((k.clone(), substitute(x, vars, interval)?))).collect::<Result<_, String>>()?,
        ),
        other => other.clone(),
    })
}

/// The concrete strategy for one variant, timeframe and cost scenario.
pub fn instantiate(
    family: &Family,
    variant: &Variant,
    interval: Interval,
    cost: &CostCase,
) -> Result<Strategy, String> {
    let mut v = substitute(&family.strategy, &variant.values, interval)?;
    let labels: Vec<String> = variant.params.iter().map(|(k, l)| format!("{k} {l}")).collect();
    if let Some(m) = v.as_object_mut() {
        m.insert("name".into(), Value::from(format!("{} ({})", family.name, labels.join(", "))));
        let market = m.entry("market").or_insert_with(|| Value::Object(Default::default()));
        if let Some(mk) = market.as_object_mut() {
            mk.insert("interval".into(), serde_json::to_value(interval).map_err(|e| e.to_string())?);
        }
        let costs = m.entry("costs").or_insert_with(|| Value::Object(Default::default()));
        if let Some(c) = costs.as_object_mut() {
            c.insert("fee_bps".into(), Value::from(cost.fee_bps));
            c.insert("slippage_bps".into(), Value::from(cost.slippage_bps));
        }
    }
    let s: Strategy = serde_json::from_value(v).map_err(|e| format!("{}: {e}", family.id))?;
    let problems = s.validate();
    if problems.is_empty() { Ok(s) } else { Err(format!("{}: {}", family.id, problems.join("; "))) }
}

/// Results for one part of the period, kept as sums so markets can be pooled.
/// Serialized as `[trades, wins, sum_r, r_trades, gross_win, gross_loss,
/// return_pct]` to keep study files small.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(from = "PartRow", into = "PartRow")]
pub struct Part {
    pub trades: usize,
    pub wins: usize,
    pub sum_r: f64,
    /// Trades with a known risk, the denominator of `sum_r`.
    pub r_trades: usize,
    pub gross_win: f64,
    pub gross_loss: f64,
    /// Sum of trade PnL as a % of starting capital.
    pub return_pct: f64,
}

type PartRow = (usize, usize, f64, usize, f64, f64, f64);

impl From<PartRow> for Part {
    fn from((trades, wins, sum_r, r_trades, gross_win, gross_loss, return_pct): PartRow) -> Self {
        Part { trades, wins, sum_r, r_trades, gross_win, gross_loss, return_pct }
    }
}

impl From<Part> for PartRow {
    fn from(p: Part) -> Self {
        (p.trades, p.wins, p.sum_r, p.r_trades, p.gross_win, p.gross_loss, p.return_pct)
    }
}

impl Part {
    fn add(&mut self, t: &Trade, capital: f64) {
        self.trades += 1;
        if t.pnl > 0.0 {
            self.wins += 1;
            self.gross_win += t.pnl;
        } else {
            self.gross_loss -= t.pnl;
        }
        if let Some(r) = t.r_multiple {
            self.sum_r += r;
            self.r_trades += 1;
        }
        self.return_pct += 100.0 * t.pnl / capital;
    }

    fn merge(&mut self, o: &Part) {
        self.trades += o.trades;
        self.wins += o.wins;
        self.sum_r += o.sum_r;
        self.r_trades += o.r_trades;
        self.gross_win += o.gross_win;
        self.gross_loss += o.gross_loss;
        self.return_pct += o.return_pct;
    }

    pub fn avg_r(&self) -> Option<f64> {
        (self.r_trades > 0).then(|| self.sum_r / self.r_trades as f64)
    }

    pub fn profit_factor(&self) -> Option<f64> {
        (self.gross_loss > 0.0).then(|| self.gross_win / self.gross_loss)
    }

    pub fn win_rate_pct(&self) -> f64 {
        if self.trades == 0 { 0.0 } else { 100.0 * self.wins as f64 / self.trades as f64 }
    }
}

/// Trades split by what the market was doing when they were opened, judged by
/// its change over the previous 24 hours: up more than [`TREND_THRESHOLD_PCT`],
/// down more than it, or in between.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ByTrend {
    pub up: Part,
    pub down: Part,
    pub sideways: Part,
}

pub const TREND_THRESHOLD_PCT: f64 = 1.5;

impl ByTrend {
    fn merge(&mut self, o: &ByTrend) {
        self.up.merge(&o.up);
        self.down.merge(&o.down);
        self.sideways.merge(&o.sideways);
    }
}

/// One variant on one market.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MarketResult {
    pub symbol: String,
    pub days: f64,
    pub in_sample: Part,
    pub out_of_sample: Part,
    /// The whole period, by market trend at entry. Only the pooled figures are
    /// kept in study files.
    #[serde(skip)]
    pub by_trend: ByTrend,
    pub max_drawdown_pct: f64,
    /// Average fees per trade as a multiple of the trade's risk.
    pub fees_r: Option<f64>,
}

impl MarketResult {
    pub fn trades_per_day(&self) -> f64 {
        (self.in_sample.trades + self.out_of_sample.trades) as f64 / self.days.max(1e-9)
    }
}

/// Runs one strategy and splits its trades at `split_ts`.
pub fn evaluate(
    strategy: &Strategy,
    symbol: &str,
    candles: &[Candle],
    capital: f64,
    interval: Interval,
    split_ts: i64,
) -> Result<MarketResult, Vec<String>> {
    let cfg = BacktestConfig { capital, interval, record_indicators: false, ..BacktestConfig::default() };
    let r = backtest::run(strategy, candles, &cfg)?;
    let (mut is, mut oos) = (Part::default(), Part::default());
    let mut by_trend = ByTrend::default();
    let (mut fee_r, mut fee_n) = (0.0, 0usize);
    let day = (86_400_000 / interval.millis()).max(1) as usize;
    for t in &r.trades {
        if t.exit_ts < split_ts {
            is.add(t, capital)
        } else {
            oos.add(t, capital)
        }
        // The market's 24-hour change up to the candle before the entry.
        let i = candles.partition_point(|c| c.ts < t.entry_ts);
        if i > day {
            let (now, then) = (candles[i - 1].close, candles[i - 1 - day].close);
            let change = 100.0 * (now / then - 1.0);
            let part = if change > TREND_THRESHOLD_PCT {
                &mut by_trend.up
            } else if change < -TREND_THRESHOLD_PCT {
                &mut by_trend.down
            } else {
                &mut by_trend.sideways
            };
            part.add(t, capital);
        }
        if let Some(rm) = t.r_multiple.filter(|r| r.abs() > 1e-12) {
            let risk = t.pnl / rm;
            if risk > 0.0 {
                fee_r += t.fees / risk;
                fee_n += 1;
            }
        }
    }
    let span = candles.last().map_or(0, |c| c.ts) - candles.first().map_or(0, |c| c.ts);
    Ok(MarketResult {
        symbol: symbol.to_string(),
        days: span as f64 / 86_400_000.0,
        in_sample: is,
        out_of_sample: oos,
        by_trend,
        max_drawdown_pct: r.metrics.max_drawdown_pct,
        fees_r: (fee_n > 0).then(|| fee_r / fee_n as f64),
    })
}

/// One variant on one timeframe and cost scenario, pooled over markets.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Summary {
    pub family: String,
    pub variant: usize,
    pub params: BTreeMap<String, String>,
    pub interval: Interval,
    pub cost: usize,
    pub markets: Vec<MarketResult>,
    pub in_sample: Part,
    pub out_of_sample: Part,
    pub by_trend: ByTrend,
    /// Average per market.
    pub trades_per_day: f64,
    /// All markets together.
    pub portfolio_trades_per_day: f64,
    pub markets_up_in_sample: usize,
    pub markets_up_out_of_sample: usize,
    pub fees_r: Option<f64>,
    pub selected: bool,
    pub survived: bool,
}

pub fn summarize(
    family: &str,
    variant: &Variant,
    interval: Interval,
    cost: usize,
    markets: Vec<MarketResult>,
    sel: &Selection,
) -> Summary {
    let (mut is, mut oos) = (Part::default(), Part::default());
    let mut by_trend = ByTrend::default();
    for m in &markets {
        is.merge(&m.in_sample);
        oos.merge(&m.out_of_sample);
        by_trend.merge(&m.by_trend);
    }
    let n = markets.len().max(1) as f64;
    let portfolio_trades_per_day = markets.iter().map(MarketResult::trades_per_day).sum::<f64>();
    let trades_per_day = portfolio_trades_per_day / n;
    let tpd = if sel.portfolio { portfolio_trades_per_day } else { trades_per_day };
    let up_is = markets.iter().filter(|m| m.in_sample.return_pct > 0.0).count();
    let up_oos = markets.iter().filter(|m| m.out_of_sample.return_pct > 0.0).count();
    let fees: Vec<f64> = markets.iter().filter_map(|m| m.fees_r).collect();
    let selected = tpd >= sel.min_trades_per_day
        && tpd <= sel.max_trades_per_day
        && is.avg_r().is_some_and(|r| r >= sel.min_avg_r)
        && up_is >= sel.min_markets.min(markets.len());
    let survived = selected && oos.avg_r().is_some_and(|r| r > 0.0) && up_oos >= sel.min_markets.min(markets.len());
    Summary {
        family: family.to_string(),
        variant: variant.index,
        params: variant.params.clone(),
        interval,
        cost,
        in_sample: is,
        out_of_sample: oos,
        by_trend,
        trades_per_day,
        portfolio_trades_per_day,
        markets_up_in_sample: up_is,
        markets_up_out_of_sample: up_oos,
        fees_r: (!fees.is_empty()).then(|| fees.iter().sum::<f64>() / fees.len() as f64),
        markets,
        selected,
        survived,
    }
}

/// The candles a study ran on.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DataRange {
    pub symbol: String,
    pub interval: Interval,
    pub from: i64,
    pub to: i64,
    pub bars: usize,
}

/// Everything a study produced: its definition, the data, and every summary.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StudyResult {
    pub study: Study,
    pub generated_at: i64,
    pub split_ts: i64,
    pub data: Vec<DataRange>,
    pub summaries: Vec<Summary>,
    /// Templates that couldn't be turned into a valid strategy.
    #[serde(default)]
    pub errors: Vec<String>,
}

/// One summary without its per-market detail, serialized as an array.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(from = "RowTuple", into = "RowTuple")]
pub struct Row {
    /// Index into the study's families.
    pub family: usize,
    pub variant: usize,
    pub interval: Interval,
    pub cost: usize,
    pub trades_per_day: f64,
    pub portfolio_trades_per_day: f64,
    pub in_sample: Part,
    pub out_of_sample: Part,
    pub markets_up_in_sample: usize,
    pub markets_up_out_of_sample: usize,
    pub markets: usize,
    pub fees_r: Option<f64>,
    pub selected: bool,
    pub survived: bool,
}

type RowTuple = (usize, usize, Interval, usize, f64, f64, Part, Part, usize, usize, usize, Option<f64>, bool, bool);

impl From<RowTuple> for Row {
    fn from(t: RowTuple) -> Self {
        Row {
            family: t.0,
            variant: t.1,
            interval: t.2,
            cost: t.3,
            trades_per_day: t.4,
            portfolio_trades_per_day: t.5,
            in_sample: t.6,
            out_of_sample: t.7,
            markets_up_in_sample: t.8,
            markets_up_out_of_sample: t.9,
            markets: t.10,
            fees_r: t.11,
            selected: t.12,
            survived: t.13,
        }
    }
}

impl From<Row> for RowTuple {
    fn from(r: Row) -> Self {
        (
            r.family,
            r.variant,
            r.interval,
            r.cost,
            r.trades_per_day,
            r.portfolio_trades_per_day,
            r.in_sample,
            r.out_of_sample,
            r.markets_up_in_sample,
            r.markets_up_out_of_sample,
            r.markets,
            r.fees_r,
            r.selected,
            r.survived,
        )
    }
}

/// A compact form of a [`StudyResult`] that the app ships: every variant's
/// pooled figures, and full detail for the selected ones. Detail for any other
/// variant can be recomputed from the recorded data window.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Digest {
    pub study: Study,
    pub generated_at: i64,
    pub split_ts: i64,
    pub data: Vec<DataRange>,
    /// Setting labels of every variant, by family.
    pub variants: Vec<Vec<BTreeMap<String, String>>>,
    pub rows: Vec<Row>,
    pub details: Vec<Summary>,
    #[serde(default)]
    pub errors: Vec<String>,
}

impl Digest {
    pub fn new(r: &StudyResult) -> Self {
        let fam = |id: &str| r.study.families.iter().position(|f| f.id == id).unwrap_or(0);
        Digest {
            study: r.study.clone(),
            generated_at: r.generated_at,
            split_ts: r.split_ts,
            data: r.data.clone(),
            variants: r
                .study
                .families
                .iter()
                .map(|f| variants(&r.study, f).into_iter().map(|v| v.params).collect())
                .collect(),
            rows: r
                .summaries
                .iter()
                .map(|s| Row {
                    family: fam(&s.family),
                    variant: s.variant,
                    interval: s.interval,
                    cost: s.cost,
                    trades_per_day: s.trades_per_day,
                    portfolio_trades_per_day: s.portfolio_trades_per_day,
                    in_sample: s.in_sample.clone(),
                    out_of_sample: s.out_of_sample.clone(),
                    markets_up_in_sample: s.markets_up_in_sample,
                    markets_up_out_of_sample: s.markets_up_out_of_sample,
                    markets: s.markets.len(),
                    fees_r: s.fees_r,
                    selected: s.selected,
                    survived: s.survived,
                })
                .collect(),
            details: r.summaries.iter().filter(|s| s.selected || r.summaries.len() <= 50).cloned().collect(),
            errors: r.errors.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn study() -> Study {
        serde_json::from_value(json!({
            "name": "t", "symbols": ["X"], "intervals": ["5m"], "days": 1, "offset_days": 0,
            "costs": [{ "label": "c", "fee_bps": 5, "slippage_bps": 1 }],
            "grid": { "rr": [1, 2], "unused": [1, 2, 3] },
            "families": [{
                "id": "f", "name": "F", "idea": "test",
                "grid": { "stop": [{ "label": "tight", "v": { "percent": 0.3 } }, { "label": "wide", "v": { "percent": 1 } }], "k": [0.5] },
                "strategy": {
                    "name": "x",
                    "entry": { "long": { "left": "close", "op": ">", "right": "open + {{k}} * range[1]" } },
                    "exit": { "stop_loss": "{{stop.v}}", "take_profit": { "risk_multiple": "{{rr}}" }, "max_bars": "{{hours:2}}" }
                }
            }]
        }))
        .unwrap()
    }

    #[test]
    fn expands_only_the_settings_a_template_uses() {
        let s = study();
        let v = variants(&s, &s.families[0]);
        assert_eq!(v.len(), 4, "rr × stop × k; `unused` doesn't multiply");
        assert_eq!(v[3].params["stop"], "wide");
        assert_eq!(v[3].params["rr"], "2");
    }

    #[test]
    fn fills_placeholders_and_costs() {
        let s = study();
        let v = &variants(&s, &s.families[0])[1];
        let st = instantiate(&s.families[0], v, Interval::M5, &s.costs[0]).unwrap();
        assert_eq!(st.exit.max_bars, Some(24), "2 hours of 5-minute candles");
        assert_eq!(st.costs.fee_bps, 5.0);
        let json = serde_json::to_value(&st).unwrap();
        assert_eq!(json["entry"]["long"]["right"], "open + 0.5 * range[1]");
        assert_eq!(json["exit"]["take_profit"]["risk_multiple"], 1.0);
        assert!(st.name.contains("stop wide") || st.name.contains("stop tight"));
    }

    #[test]
    fn unknown_settings_are_errors() {
        let s = study();
        let mut f = s.families[0].clone();
        f.strategy["exit"]["max_bars"] = json!("{{nope}}");
        let v = &variants(&s, &f)[0];
        assert!(instantiate(&f, v, Interval::M5, &s.costs[0]).unwrap_err().contains("nope"));
    }

    #[test]
    fn parts_and_rows_round_trip_as_arrays() {
        let p =
            Part { trades: 3, wins: 2, sum_r: 1.5, r_trades: 3, gross_win: 30.0, gross_loss: 10.0, return_pct: 0.2 };
        let j = serde_json::to_string(&p).unwrap();
        assert_eq!(j, "[3,2,1.5,3,30.0,10.0,0.2]");
        assert_eq!(serde_json::from_str::<Part>(&j).unwrap(), p);
    }

    #[test]
    fn trades_are_tagged_by_the_market_trend() {
        // A 5-minute series that rises 3% over a day, then trades sideways.
        let day = 288;
        let mut candles = vec![];
        for i in 0..(day * 3) {
            let px =
                if i < day { 100.0 + 3.0 * i as f64 / day as f64 } else { 103.0 + if i % 2 == 0 { 0.1 } else { -0.1 } };
            candles.push(Candle {
                ts: i as i64 * 300_000,
                open: px,
                high: px + 0.2,
                low: px - 0.2,
                close: px,
                volume: 1.0,
            });
        }
        let s = Strategy::from_json(
            r#"{ "name": "every 50th", "entry": { "long": { "left": "close", "op": ">", "right": 0 } },
                 "exit": { "max_bars": 1, "stop_loss": { "percent": 1 } }, "sizing": { "type": "risk_percent", "value": 1 } }"#,
        )
        .unwrap();
        let m = evaluate(&s, "X", &candles, 10_000.0, Interval::M5, i64::MAX).unwrap();
        assert!(m.by_trend.up.trades > 0, "trades just after the rally count as uptrend");
        assert!(m.by_trend.sideways.trades > 0);
        assert_eq!(m.by_trend.down.trades, 0);
        let total = m.by_trend.up.trades + m.by_trend.sideways.trades;
        assert!(total < m.in_sample.trades, "the first day has no 24-hour history");
    }

    #[test]
    fn pooling_and_selection() {
        let part = |trades: usize, sum_r: f64, ret: f64| Part {
            trades,
            wins: trades / 2,
            sum_r,
            r_trades: trades,
            gross_win: 2.0,
            gross_loss: 1.0,
            return_pct: ret,
        };
        let m = |sym: &str, is: Part, oos: Part| MarketResult {
            symbol: sym.into(),
            days: 10.0,
            in_sample: is,
            out_of_sample: oos,
            by_trend: ByTrend::default(),
            max_drawdown_pct: 1.0,
            fees_r: Some(0.2),
        };
        let v = Variant { index: 0, params: BTreeMap::new(), values: BTreeMap::new() };
        let sel = Selection { min_markets: 2, ..Selection::default() };
        let s = summarize(
            "f",
            &v,
            Interval::M5,
            0,
            vec![m("A", part(20, 4.0, 3.0), part(10, 1.0, 1.0)), m("B", part(20, 2.0, 1.0), part(10, 0.5, 0.5))],
            &sel,
        );
        assert_eq!(s.trades_per_day, 3.0);
        assert!((s.in_sample.avg_r().unwrap() - 0.15).abs() < 1e-12);
        assert!(s.selected && s.survived);
        let s2 = summarize(
            "f",
            &v,
            Interval::M5,
            0,
            vec![m("A", part(20, 4.0, 3.0), part(10, -3.0, -1.0)), m("B", part(20, 2.0, 1.0), part(10, 0.5, 0.5))],
            &sel,
        );
        assert!(s2.selected && !s2.survived, "picked in sample, failed out of sample");
    }
}
