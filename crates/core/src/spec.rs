//! The strategy file: a small JSON document that describes a whole strategy.
//!
//! ```json
//! {
//!   "name": "RSI dip in an uptrend",
//!   "indicators": { "rsi": { "type": "rsi", "period": 14 }, "trend": { "type": "sma", "period": 200 } },
//!   "entry": { "long": { "all": [
//!       { "left": "rsi", "op": "crosses_below", "right": 30 },
//!       { "left": "close", "op": ">", "right": "trend" } ] } },
//!   "exit": { "stop_loss": { "percent": 2 }, "take_profit": { "risk_multiple": 2 } },
//!   "sizing": { "type": "risk_percent", "value": 1 },
//!   "leverage": 2
//! }
//! ```
//!
//! The no-code builder, the AI assistant and hand-written files all produce
//! this format, and `schema/strategy.schema.json` describes it formally.
//!
//! Values in rules are either numbers or references: a price field (`close`,
//! `high`, `hl2`, ...), an indicator id (`rsi`), or an indicator output
//! (`macd.signal`), optionally with a lookback in brackets (`close[1]` is the
//! previous bar's close).

use crate::candle::{Candle, Interval};
use crate::indicators::{self, Indicator, IndicatorInfo};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Strategy {
    pub name: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub description: String,
    /// Background for templates: how it works, when it works and when it fails.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub about: Option<About>,
    #[serde(default)]
    pub market: Market,
    /// Indicators by the id rules use to refer to them.
    #[serde(default)]
    pub indicators: BTreeMap<String, Map<String, Value>>,
    pub entry: Sides,
    #[serde(default)]
    pub exit: Exit,
    #[serde(default)]
    pub sizing: Sizing,
    /// 1 = no leverage. Positions use isolated margin: at most the margin can be lost.
    #[serde(default = "one")]
    pub leverage: f64,
    #[serde(default)]
    pub costs: Costs,
}

fn one() -> f64 {
    1.0
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct About {
    #[serde(default)]
    pub category: String,
    #[serde(default)]
    pub timeframe: String,
    #[serde(default)]
    pub how_it_works: String,
    #[serde(default)]
    pub works_best: String,
    #[serde(default)]
    pub fails_when: String,
}

/// Default market for the strategy; the command line or UI can override it.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Market {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub symbol: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub interval: Option<Interval>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Sides {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub long: Option<Condition>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub short: Option<Condition>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Exit {
    /// Rule that closes a long position at the next bar's open.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub long: Option<Condition>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub short: Option<Condition>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stop_loss: Option<Distance>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub take_profit: Option<Target>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trailing_stop: Option<Distance>,
    /// Close after this many bars in the trade.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_bars: Option<u32>,
}

/// A price distance from the entry: a percentage, or a multiple of an ATR indicator.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Distance {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub percent: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub atr: Option<f64>,
    /// Which ATR indicator to use with `atr` (an id from `indicators`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub indicator: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Target {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub percent: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub atr: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub indicator: Option<String>,
    /// Target at this multiple of the stop distance (2 = "risk 1 to make 2").
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub risk_multiple: Option<f64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SizingType {
    /// Margin per trade as a percentage of equity. Position size = margin × leverage.
    PercentEquity,
    /// Size the position so hitting the stop loses this percentage of equity.
    RiskPercent,
    /// A fixed quantity of the asset (e.g. 0.01 BTC).
    Fixed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Sizing {
    #[serde(rename = "type")]
    pub kind: SizingType,
    pub value: f64,
}

impl Default for Sizing {
    fn default() -> Self {
        Sizing { kind: SizingType::PercentEquity, value: 100.0 }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Costs {
    /// Fee per fill in basis points of notional (10 = 0.1%, typical retail spot).
    #[serde(default = "d_fee")]
    pub fee_bps: f64,
    /// Adverse slippage on market and stop fills, in basis points.
    #[serde(default = "d_slip")]
    pub slippage_bps: f64,
    /// Maintenance margin rate used for the liquidation price (0.5 = 0.5%).
    #[serde(default = "d_mmr")]
    pub maintenance_margin_percent: f64,
}

fn d_fee() -> f64 {
    10.0
}
fn d_slip() -> f64 {
    2.0
}
fn d_mmr() -> f64 {
    0.5
}

impl Default for Costs {
    fn default() -> Self {
        Costs { fee_bps: d_fee(), slippage_bps: d_slip(), maintenance_margin_percent: d_mmr() }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Condition {
    All { all: Vec<Condition> },
    Any { any: Vec<Condition> },
    Not { not: Box<Condition> },
    Rule(Rule),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Rule {
    pub left: Operand,
    pub op: Op,
    pub right: Operand,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Operand {
    Number(f64),
    Ref(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Op {
    #[serde(rename = ">", alias = "above")]
    Gt,
    #[serde(rename = "<", alias = "below")]
    Lt,
    #[serde(rename = ">=")]
    Ge,
    #[serde(rename = "<=")]
    Le,
    #[serde(rename = "crosses_above")]
    CrossesAbove,
    #[serde(rename = "crosses_below")]
    CrossesBelow,
    /// `left` is higher than it was `right` bars ago.
    #[serde(rename = "rising")]
    Rising,
    #[serde(rename = "falling")]
    Falling,
}

pub const PRICE_FIELDS: [&str; 7] = ["open", "high", "low", "close", "volume", "hl2", "hlc3"];

impl Strategy {
    pub fn from_json(text: &str) -> Result<Self, String> {
        serde_json::from_str(text).map_err(|e| format!("not a valid strategy file: {e}"))
    }

    /// Like [`Strategy::from_json`], but tolerant of what chat assistants
    /// usually wrap around JSON: code fences, surrounding prose, `//` comments
    /// and trailing commas.
    pub fn from_ai_text(text: &str) -> Result<Self, String> {
        Self::from_json(&clean_json(text))
    }

    /// Checks everything that isn't a JSON shape error. An empty list means the
    /// strategy can run.
    pub fn validate(&self) -> Vec<String> {
        match compile(self) {
            Ok(_) => vec![],
            Err(errors) => errors,
        }
    }
}

/// Extracts the outermost `{...}`, drops `//` comments and trailing commas.
pub fn clean_json(text: &str) -> String {
    let (Some(a), Some(b)) = (text.find('{'), text.rfind('}')) else { return text.to_string() };
    let body = &text[a..=b];
    let mut out = String::with_capacity(body.len());
    let mut chars = body.chars().peekable();
    let mut in_str = false;
    while let Some(c) = chars.next() {
        if in_str {
            out.push(c);
            if c == '\\' {
                if let Some(n) = chars.next() {
                    out.push(n);
                }
            } else if c == '"' {
                in_str = false;
            }
            continue;
        }
        match c {
            '"' => {
                in_str = true;
                out.push(c);
            }
            '/' if chars.peek() == Some(&'/') => {
                for n in chars.by_ref() {
                    if n == '\n' {
                        out.push('\n');
                        break;
                    }
                }
            }
            ',' => {
                // Drop a comma that only has whitespace before the closing bracket.
                let rest: String = chars.clone().take_while(|x| x.is_whitespace()).collect();
                let next = chars.clone().nth(rest.chars().count());
                if !matches!(next, Some('}') | Some(']')) {
                    out.push(c);
                }
            }
            _ => out.push(c),
        }
    }
    out
}

// ---------------------------------------------------------------------------
// Compilation: resolve every reference once, so evaluation is index lookups.
// ---------------------------------------------------------------------------

/// Where a value comes from: a price field or an indicator output.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Source {
    Price(usize),
    Indicator { slot: usize, output: usize },
}

#[derive(Debug, Clone, Copy)]
pub enum Val {
    Const(f64),
    Series { series: usize, offset: usize },
}

#[derive(Debug, Clone)]
pub enum Cond {
    All(Vec<Cond>),
    Any(Vec<Cond>),
    Not(Box<Cond>),
    Cmp { left: Val, op: Op, right: Val },
}

pub struct IndicatorSlot {
    pub id: String,
    pub info: &'static IndicatorInfo,
    pub params: Vec<f64>,
    pub label: String,
    pub ind: Box<dyn Indicator>,
}

/// A strategy with references resolved, ready to run bar by bar.
pub struct Compiled {
    pub slots: Vec<IndicatorSlot>,
    /// Every value series the rules read, in order of first use.
    pub sources: Vec<Source>,
    pub entry_long: Option<Cond>,
    pub entry_short: Option<Cond>,
    pub exit_long: Option<Cond>,
    pub exit_short: Option<Cond>,
    /// Series index of the ATR used by stops/targets, if any.
    pub stop_atr: Option<usize>,
    pub trail_atr: Option<usize>,
    pub target_atr: Option<usize>,
    /// Longest lookback any rule needs (for history retention).
    pub max_offset: usize,
}

struct Ctx<'a> {
    strat: &'a Strategy,
    slots: Vec<IndicatorSlot>,
    sources: Vec<Source>,
    errors: Vec<String>,
    max_offset: usize,
}

impl Ctx<'_> {
    fn source_index(&mut self, s: Source) -> usize {
        if let Some(i) = self.sources.iter().position(|x| *x == s) {
            return i;
        }
        self.sources.push(s);
        self.sources.len() - 1
    }

    fn known(&self) -> String {
        let ids: Vec<&str> = self.slots.iter().map(|s| s.id.as_str()).collect();
        format!(
            "indicators: {}; price fields: {}",
            if ids.is_empty() { "none defined".to_string() } else { ids.join(", ") },
            PRICE_FIELDS.join(", ")
        )
    }

    /// `close`, `rsi`, `macd.signal`, `bb.upper[2]`
    fn reference(&mut self, text: &str, at: &str) -> Option<Val> {
        let t = text.trim();
        let (name, offset) = match t.split_once('[') {
            Some((n, rest)) => match rest.strip_suffix(']').and_then(|o| o.trim().parse::<usize>().ok()) {
                Some(o) => (n.trim(), o),
                None => {
                    self.errors.push(format!("{at}: bad lookback in `{t}` (write e.g. close[1])"));
                    return None;
                }
            },
            None => (t, 0),
        };
        self.max_offset = self.max_offset.max(offset + 1);
        if let Some(i) = PRICE_FIELDS.iter().position(|f| *f == name) {
            let series = self.source_index(Source::Price(i));
            return Some(Val::Series { series, offset });
        }
        let (id, output) = match name.split_once('.') {
            Some((a, b)) => (a, Some(b)),
            None => (name, None),
        };
        let Some(slot) = self.slots.iter().position(|s| s.id == id) else {
            self.errors.push(format!("{at}: unknown value `{name}` ({})", self.known()));
            return None;
        };
        let info = self.slots[slot].info;
        let out = match output {
            None => 0,
            Some(o) => match info.outputs.iter().position(|x| *x == o) {
                Some(i) => i,
                None => {
                    self.errors.push(format!(
                        "{at}: {id} ({}) has no output `{o}` (outputs: {})",
                        info.kind,
                        info.outputs.join(", ")
                    ));
                    return None;
                }
            },
        };
        let series = self.source_index(Source::Indicator { slot, output: out });
        Some(Val::Series { series, offset })
    }

    fn operand(&mut self, o: &Operand, at: &str) -> Option<Val> {
        match o {
            Operand::Number(n) => Some(Val::Const(*n)),
            Operand::Ref(s) => match s.trim().parse::<f64>() {
                Ok(n) => Some(Val::Const(n)),
                Err(_) => self.reference(s, at),
            },
        }
    }

    fn cond(&mut self, c: &Condition, at: &str) -> Option<Cond> {
        match c {
            Condition::All { all } | Condition::Any { any: all } if all.is_empty() => {
                self.errors.push(format!("{at}: empty group"));
                None
            }
            Condition::All { all } => {
                let v: Vec<Option<Cond>> =
                    all.iter().enumerate().map(|(i, c)| self.cond(c, &format!("{at}.all[{i}]"))).collect();
                v.into_iter().collect::<Option<Vec<_>>>().map(Cond::All)
            }
            Condition::Any { any } => {
                let v: Vec<Option<Cond>> =
                    any.iter().enumerate().map(|(i, c)| self.cond(c, &format!("{at}.any[{i}]"))).collect();
                v.into_iter().collect::<Option<Vec<_>>>().map(Cond::Any)
            }
            Condition::Not { not } => self.cond(not, &format!("{at}.not")).map(|c| Cond::Not(Box::new(c))),
            Condition::Rule(r) => {
                let left = self.operand(&r.left, at);
                if matches!(r.op, Op::Rising | Op::Falling) {
                    let n = match &r.right {
                        Operand::Number(n) if *n >= 1.0 && n.fract() == 0.0 => *n as usize,
                        _ => {
                            self.errors.push(
                                format!("{at}: `{:?}` needs a whole number of bars on the right", r.op).to_lowercase(),
                            );
                            return None;
                        }
                    };
                    self.max_offset = self.max_offset.max(n + 1);
                    return Some(Cond::Cmp { left: left?, op: r.op, right: Val::Const(n as f64) });
                }
                let right = self.operand(&r.right, at);
                if matches!(r.op, Op::CrossesAbove | Op::CrossesBelow) {
                    self.max_offset = self.max_offset.max(2);
                }
                Some(Cond::Cmp { left: left?, op: r.op, right: right? })
            }
        }
    }

    fn atr_ref(&mut self, d_atr: Option<f64>, ind: &Option<String>, at: &str) -> Option<usize> {
        d_atr?;
        let Some(id) = ind else {
            self.errors.push(format!("{at}: `atr` needs `indicator`, the id of an ATR indicator"));
            return None;
        };
        match self.reference(id, at)? {
            Val::Series { series, .. } => Some(series),
            Val::Const(_) => None,
        }
    }
}

fn check_distance(d: &Distance, at: &str, errors: &mut Vec<String>) {
    match (d.percent, d.atr) {
        (Some(p), None) if p > 0.0 => {}
        (None, Some(a)) if a > 0.0 => {}
        (Some(_), Some(_)) => errors.push(format!("{at}: use either `percent` or `atr`, not both")),
        (None, None) => errors.push(format!("{at}: needs `percent` or `atr`")),
        _ => errors.push(format!("{at}: must be positive")),
    }
}

pub fn compile(s: &Strategy) -> Result<Compiled, Vec<String>> {
    let mut cx = Ctx { strat: s, slots: vec![], sources: vec![], errors: vec![], max_offset: 1 };
    for (id, def) in &s.indicators {
        if PRICE_FIELDS.contains(&id.as_str()) || id.contains(['.', '[', ']', ' ']) || id.is_empty() {
            cx.errors
                .push(format!("indicators.{id}: pick another id (not a price field, no dots, spaces or brackets)"));
            continue;
        }
        let Some(kind) = def.get("type").and_then(|v| v.as_str()) else {
            cx.errors.push(format!("indicators.{id}: needs \"type\", e.g. \"rsi\""));
            continue;
        };
        let Some(info) = indicators::info(kind) else {
            let known: Vec<&str> = indicators::CATALOG.iter().map(|i| i.kind).collect();
            cx.errors.push(format!("indicators.{id}: unknown type `{kind}` (available: {})", known.join(", ")));
            continue;
        };
        match indicators::resolve_params(info, def) {
            Ok(params) => {
                let ind = indicators::build(kind, &params).expect("catalog kinds all build");
                let label = indicators::label(info, &params);
                cx.slots.push(IndicatorSlot { id: id.clone(), info, params, label, ind });
            }
            Err(e) => cx.errors.push(format!("indicators.{id}: {e}")),
        }
    }

    let entry_long = s.entry.long.as_ref().and_then(|c| cx.cond(c, "entry.long"));
    let entry_short = s.entry.short.as_ref().and_then(|c| cx.cond(c, "entry.short"));
    if s.entry.long.is_none() && s.entry.short.is_none() {
        cx.errors.push("entry: needs a `long` or `short` rule".into());
    }
    let exit_long = s.exit.long.as_ref().and_then(|c| cx.cond(c, "exit.long"));
    let exit_short = s.exit.short.as_ref().and_then(|c| cx.cond(c, "exit.short"));

    let x = &s.exit;
    let mut errs = vec![];
    if let Some(d) = &x.stop_loss {
        check_distance(d, "exit.stop_loss", &mut errs);
    }
    if let Some(d) = &x.trailing_stop {
        check_distance(d, "exit.trailing_stop", &mut errs);
    }
    if let Some(t) = &x.take_profit {
        let set = [t.percent.is_some(), t.atr.is_some(), t.risk_multiple.is_some()].iter().filter(|b| **b).count();
        if set != 1 {
            errs.push("exit.take_profit: use exactly one of `percent`, `atr` or `risk_multiple`".into());
        }
        if t.risk_multiple.is_some() && x.stop_loss.is_none() {
            errs.push("exit.take_profit: `risk_multiple` needs a stop_loss to measure risk against".into());
        }
    }
    if x.max_bars == Some(0) {
        errs.push("exit.max_bars: must be at least 1".into());
    }
    let has_exit = x.stop_loss.is_some()
        || x.take_profit.is_some()
        || x.trailing_stop.is_some()
        || x.max_bars.is_some()
        || (s.entry.long.is_none() || x.long.is_some()) && (s.entry.short.is_none() || x.short.is_some());
    if !has_exit {
        errs.push(
            "exit: add a stop_loss, take_profit, trailing_stop, max_bars or an exit rule for each side you enter"
                .into(),
        );
    }
    match s.sizing.kind {
        SizingType::RiskPercent if x.stop_loss.is_none() => {
            errs.push("sizing: `risk_percent` needs exit.stop_loss (risk is measured to the stop)".into())
        }
        SizingType::PercentEquity if !(0.0..=100.0).contains(&s.sizing.value) || s.sizing.value == 0.0 => {
            errs.push("sizing.value: percent_equity must be between 0 and 100".into())
        }
        SizingType::RiskPercent if !(0.0..=100.0).contains(&s.sizing.value) || s.sizing.value == 0.0 => {
            errs.push("sizing.value: risk_percent must be between 0 and 100".into())
        }
        _ if s.sizing.value <= 0.0 => errs.push("sizing.value: must be positive".into()),
        _ => {}
    }
    if !(1.0..=125.0).contains(&s.leverage) {
        errs.push("leverage: must be between 1 and 125".into());
    }
    if s.costs.fee_bps < 0.0 || s.costs.slippage_bps < 0.0 {
        errs.push("costs: fees and slippage can't be negative".into());
    }
    cx.errors.extend(errs);

    let stop_atr = x.stop_loss.as_ref().and_then(|d| cx.atr_ref(d.atr, &d.indicator, "exit.stop_loss"));
    let trail_atr = x.trailing_stop.as_ref().and_then(|d| cx.atr_ref(d.atr, &d.indicator, "exit.trailing_stop"));
    let target_atr = x.take_profit.as_ref().and_then(|t| cx.atr_ref(t.atr, &t.indicator, "exit.take_profit"));

    let _ = cx.strat;
    if !cx.errors.is_empty() {
        return Err(cx.errors);
    }
    Ok(Compiled {
        slots: cx.slots,
        sources: cx.sources,
        entry_long,
        entry_short,
        exit_long,
        exit_short,
        stop_atr,
        trail_atr,
        target_atr,
        max_offset: cx.max_offset,
    })
}

// ---------------------------------------------------------------------------
// Evaluation against the history of values.
// ---------------------------------------------------------------------------

/// Per-series history, one value per bar (NaN while an indicator warms up).
pub struct History {
    pub series: Vec<Vec<f64>>,
}

impl History {
    pub fn new(n: usize) -> Self {
        History { series: vec![Vec::new(); n] }
    }

    pub fn len(&self) -> usize {
        self.series.first().map_or(0, Vec::len)
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    fn at(&self, v: Val, back: usize) -> Option<f64> {
        match v {
            Val::Const(c) => Some(c),
            Val::Series { series, offset } => {
                let s = &self.series[series];
                let idx = s.len().checked_sub(1 + offset + back)?;
                let x = s[idx];
                x.is_finite().then_some(x)
            }
        }
    }

    pub fn latest(&self, series: usize) -> Option<f64> {
        let x = *self.series[series].last()?;
        x.is_finite().then_some(x)
    }
}

impl Compiled {
    /// Feeds a closed candle to every indicator and records the new values.
    pub fn push(&mut self, c: &Candle, h: &mut History) {
        for s in &mut self.slots {
            s.ind.update(c);
        }
        for (i, src) in self.sources.iter().enumerate() {
            let v = match *src {
                Source::Price(f) => match f {
                    0 => c.open,
                    1 => c.high,
                    2 => c.low,
                    3 => c.close,
                    4 => c.volume,
                    5 => c.hl2(),
                    _ => c.hlc3(),
                },
                Source::Indicator { slot, output } => self.slots[slot].ind.get(output).unwrap_or(f64::NAN),
            };
            h.series[i].push(v);
        }
    }
}

/// True when the condition holds on the latest bar. Missing values (warm-up,
/// not enough history) make a rule false.
pub fn eval(c: &Cond, h: &History) -> bool {
    match c {
        Cond::All(v) => v.iter().all(|c| eval(c, h)),
        Cond::Any(v) => v.iter().any(|c| eval(c, h)),
        Cond::Not(c) => !eval(c, h),
        Cond::Cmp { left, op, right } => {
            let cmp = |back: usize| Some((h.at(*left, back)?, h.at(*right, back)?));
            match op {
                Op::Gt => cmp(0).is_some_and(|(l, r)| l > r),
                Op::Lt => cmp(0).is_some_and(|(l, r)| l < r),
                Op::Ge => cmp(0).is_some_and(|(l, r)| l >= r),
                Op::Le => cmp(0).is_some_and(|(l, r)| l <= r),
                Op::CrossesAbove => matches!((cmp(0), cmp(1)), (Some((l, r)), Some((pl, pr))) if l > r && pl <= pr),
                Op::CrossesBelow => matches!((cmp(0), cmp(1)), (Some((l, r)), Some((pl, pr))) if l < r && pl >= pr),
                Op::Rising | Op::Falling => {
                    let n = match right {
                        Val::Const(n) => *n as usize,
                        _ => return false,
                    };
                    match (h.at(*left, 0), h.at(*left, n)) {
                        (Some(now), Some(then)) => {
                            if *op == Op::Rising {
                                now > then
                            } else {
                                now < then
                            }
                        }
                        _ => false,
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const RSI: &str = r#"{
        "name": "t",
        "indicators": { "rsi": { "type": "rsi", "period": 2 }, "fast": { "type": "sma", "period": 2 } },
        "entry": { "long": { "all": [ { "left": "rsi", "op": "<", "right": 50 }, { "left": "close", "op": "rising", "right": 1 } ] } },
        "exit": { "stop_loss": { "percent": 2 } }
    }"#;

    #[test]
    fn parses_and_compiles() {
        let s = Strategy::from_json(RSI).unwrap();
        assert!(s.validate().is_empty(), "{:?}", s.validate());
        assert_eq!(s.leverage, 1.0);
        assert_eq!(s.costs.fee_bps, 10.0);
    }

    #[test]
    fn reports_helpful_errors() {
        let bad = r#"{
            "name": "t",
            "indicators": { "rsi": { "type": "rsii" }, "m": { "type": "macd" } },
            "entry": { "long": { "left": "m.sig", "op": "crosses_above", "right": "clsoe[1]" } },
            "sizing": { "type": "risk_percent", "value": 1 },
            "leverage": 500
        }"#;
        let errs = Strategy::from_json(bad).unwrap().validate().join("\n");
        for needle in [
            "unknown type `rsii`",
            "no output `sig`",
            "unknown value `clsoe`",
            "needs exit.stop_loss",
            "leverage",
            "exit:",
        ] {
            assert!(errs.contains(needle), "missing `{needle}` in:\n{errs}");
        }
    }

    #[test]
    fn accepts_chat_style_json() {
        let text = "Here you go:\n```json\n{ \"name\": \"a // b\", // comment\n  \"entry\": { \"long\": { \"left\": \"close\", \"op\": \">\", \"right\": 1, } },\n  \"exit\": { \"max_bars\": 2 }, }\n```\nGood luck!";
        let s = Strategy::from_ai_text(text).unwrap();
        assert_eq!(s.name, "a // b", "comment markers inside strings are kept");
        assert!(s.validate().is_empty());
    }

    #[test]
    fn crosses_and_lookbacks() {
        let s = Strategy::from_json(
            r#"{ "name": "x", "entry": { "long": { "left": "close", "op": "crosses_above", "right": "close[1]" } }, "exit": { "max_bars": 1 } }"#,
        )
        .unwrap();
        let mut c = compile(&s).unwrap();
        let mut h = History::new(c.sources.len());
        let bar = |close: f64| Candle { ts: 0, open: close, high: close, low: close, close, volume: 1.0 };
        let mut fired = vec![];
        for x in [5.0, 4.0, 3.0, 4.0, 5.0] {
            c.push(&bar(x), &mut h);
            fired.push(eval(c.entry_long.as_ref().unwrap(), &h));
        }
        // close > previous close, having been <= on the bar before: only at the turn.
        assert_eq!(fired, vec![false, false, false, true, false]);
    }
}
