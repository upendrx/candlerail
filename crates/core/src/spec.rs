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
    /// Account-level money-management rules.
    #[serde(default, skip_serializing_if = "RiskRules::is_empty")]
    pub risk: RiskRules,
}

/// Money-management guards applied to the whole account, on top of each
/// trade's own stop. When a limit is hit, open positions are closed at the
/// next open and new entries are blocked for as long as the rule says.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RiskRules {
    /// Stop trading for good once the account is this far below its peak.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_drawdown_percent: Option<f64>,
    /// No new trades for the rest of the UTC day after losing this much of the day's starting balance.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub daily_loss_percent: Option<f64>,
    /// The same for the calendar month (Elder's "6% rule").
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub monthly_loss_percent: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_trades_per_day: Option<u32>,
    /// After this many losing trades in a row, stop for `pause_bars` candles.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pause_after_losses: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pause_bars: Option<u32>,
}

impl RiskRules {
    pub fn is_empty(&self) -> bool {
        *self == RiskRules::default()
    }
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
    /// For longs: a price level, read when the signal fires, e.g. `"low"` or `"swings.support"`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub below: Option<String>,
    /// For shorts: a price level above the entry, e.g. `"high"` or `"swings.resistance"`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub above: Option<String>,
}

impl Distance {
    pub fn is_level(&self) -> bool {
        self.below.is_some() || self.above.is_some()
    }
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
    /// For longs: a price level above the entry, e.g. `"swings.resistance"`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub above: Option<String>,
    /// For shorts: a price level below the entry.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub below: Option<String>,
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
    /// Equal (within floating-point noise). Useful for pattern flags: `pa.hammer == 1`.
    #[serde(rename = "==", alias = "equals")]
    Eq,
    #[serde(rename = "!=")]
    Ne,
}

/// Values every candle has. `body` is |close − open|, `range` is high − low,
/// and the wicks are the parts of the range above and below the body.
pub const PRICE_FIELDS: [&str; 11] =
    ["open", "high", "low", "close", "volume", "hl2", "hlc3", "body", "range", "upper_wick", "lower_wick"];

impl Strategy {
    pub fn from_json(text: &str) -> Result<Self, String> {
        serde_json::from_str(text).map_err(|e| format!("not a valid strategy file: {e}"))
    }

    /// Like [`Strategy::from_json`], but tolerant of what chat assistants
    /// usually wrap around JSON: code fences, surrounding prose, `//` comments
    /// and trailing commas.
    pub fn from_ai_text(text: &str) -> Result<Self, String> {
        let cleaned = clean_json(text);
        // A share file wraps the strategy together with its recorded result.
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(&cleaned)
            && v.get("candlerail_share").is_some()
            && let Some(inner) = v.get("strategy")
        {
            return serde_json::from_value(inner.clone()).map_err(|e| format!("not a valid strategy file: {e}"));
        }
        Self::from_json(&cleaned)
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
    Series {
        series: usize,
        offset: usize,
    },
    /// Index into the compiled expressions.
    Expr(usize),
}

/// A linear expression such as `2 * body[1] + 0.5`: a sum of scaled series
/// values plus a constant.
#[derive(Debug, Clone, Default)]
pub struct Lin {
    /// (coefficient, series, lookback)
    pub terms: Vec<(f64, usize, usize)>,
    pub constant: f64,
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
    pub exprs: Vec<Lin>,
    /// Price-level stops and targets, read when an entry signal fires.
    pub stop_below: Option<Val>,
    pub stop_above: Option<Val>,
    pub target_above: Option<Val>,
    pub target_below: Option<Val>,
}

struct Ctx<'a> {
    strat: &'a Strategy,
    slots: Vec<IndicatorSlot>,
    sources: Vec<Source>,
    errors: Vec<String>,
    max_offset: usize,
    exprs: Vec<Lin>,
}

#[derive(Debug, Clone, PartialEq)]
enum Tok {
    Num(f64),
    Ref(String),
    Op(char),
}

fn tokenize(s: &str) -> Result<Vec<Tok>, String> {
    let mut out = vec![];
    let chars: Vec<char> = s.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if c.is_whitespace() {
            i += 1;
        } else if "+-*/".contains(c) {
            out.push(Tok::Op(c));
            i += 1;
        } else if c.is_ascii_digit() || c == '.' {
            let st = i;
            while i < chars.len() && (chars[i].is_ascii_digit() || chars[i] == '.') {
                i += 1;
            }
            let t: String = chars[st..i].iter().collect();
            out.push(Tok::Num(t.parse().map_err(|_| format!("bad number `{t}`"))?));
        } else if c.is_ascii_alphabetic() || c == '_' {
            let st = i;
            while i < chars.len() && (chars[i].is_ascii_alphanumeric() || "_.[]".contains(chars[i])) {
                i += 1;
            }
            out.push(Tok::Ref(chars[st..i].iter().collect()));
        } else if c == '(' || c == ')' {
            return Err("parentheses aren't supported; write it as a sum, e.g. `high[1] + 0.5 * atr`".into());
        } else {
            return Err(format!("unexpected `{c}`"));
        }
    }
    Ok(out)
}

/// True when a value string is an arithmetic expression rather than a single reference.
pub fn is_expression_text(s: &str) -> bool {
    is_expression(s)
}

fn is_expression(s: &str) -> bool {
    let t = s.trim();
    t.contains(['+', '*', '/']) || t.trim_start_matches('-').contains('-')
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
                Err(_) if is_expression(s) => self.expression(s, at),
                Err(_) => self.reference(s, at),
            },
        }
    }

    /// Sums and differences of terms, each a number, a value, or a number times
    /// (or divided into) a value: `2 * body[1]`, `high[1] + 0.25 * range`, `volume / 3`.
    fn expression(&mut self, text: &str, at: &str) -> Option<Val> {
        let toks = match tokenize(text) {
            Ok(t) => t,
            Err(e) => {
                self.errors.push(format!("{at}: in `{text}`: {e}"));
                return None;
            }
        };
        let mut lin = Lin::default();
        let mut i = 0;
        let mut sign = 1.0;
        let bad = |cx: &mut Self, why: &str| {
            cx.errors.push(format!("{at}: can't read `{text}`: {why}"));
            None
        };
        while i < toks.len() {
            if let Tok::Op(c @ ('+' | '-')) = toks[i] {
                if c == '-' {
                    sign = -sign;
                }
                i += 1;
                continue;
            }
            // One term: factors joined by * or /, at most one of them a value.
            let mut coef = sign;
            let mut series: Option<(usize, usize)> = None;
            let mut first = true;
            loop {
                let op = if first {
                    '*'
                } else {
                    match toks.get(i) {
                        Some(Tok::Op(c @ ('*' | '/'))) => {
                            i += 1;
                            *c
                        }
                        _ => break,
                    }
                };
                first = false;
                match toks.get(i).cloned() {
                    Some(Tok::Num(n)) => {
                        if op == '/' {
                            if n == 0.0 {
                                return bad(self, "division by zero");
                            }
                            coef /= n;
                        } else {
                            coef *= n;
                        }
                    }
                    Some(Tok::Ref(r)) => {
                        if op == '/' || series.is_some() {
                            return bad(self, "values can only be multiplied or divided by numbers");
                        }
                        match self.reference(&r, at)? {
                            Val::Series { series: s, offset } => series = Some((s, offset)),
                            _ => return bad(self, "unexpected value"),
                        }
                    }
                    _ => return bad(self, "expected a number or a value"),
                }
                i += 1;
            }
            match series {
                Some((s, o)) => lin.terms.push((coef, s, o)),
                None => lin.constant += coef,
            }
            sign = 1.0;
            match toks.get(i) {
                None => {}
                Some(Tok::Op('+' | '-')) => {}
                Some(_) => return bad(self, "expected + or - between terms"),
            }
        }
        if lin.terms.is_empty() {
            return Some(Val::Const(lin.constant));
        }
        self.exprs.push(lin);
        Some(Val::Expr(self.exprs.len() - 1))
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
            _ => None,
        }
    }
}

fn check_distance(d: &Distance, at: &str, s: &Strategy, levels_ok: bool, errors: &mut Vec<String>) {
    let kinds = [d.percent.is_some(), d.atr.is_some(), d.is_level()].iter().filter(|b| **b).count();
    if kinds > 1 {
        errors.push(format!("{at}: use one of `percent`, `atr`, or price levels (`below`/`above`)"));
        return;
    }
    if d.is_level() {
        if !levels_ok {
            errors.push(format!("{at}: price levels aren't supported here; use `percent` or `atr`"));
        }
        if s.entry.long.is_some() && d.below.is_none() {
            errors.push(format!("{at}: long entries need `below`, the level the stop goes at (e.g. \"low\")"));
        }
        if s.entry.short.is_some() && d.above.is_none() {
            errors.push(format!("{at}: short entries need `above`, the level the stop goes at (e.g. \"high\")"));
        }
        return;
    }
    match (d.percent, d.atr) {
        (Some(p), None) if p > 0.0 => {}
        (None, Some(a)) if a > 0.0 => {}
        (None, None) => errors.push(format!("{at}: needs `percent`, `atr`, or a price level (`below`/`above`)")),
        _ => errors.push(format!("{at}: must be positive")),
    }
}

pub fn compile(s: &Strategy) -> Result<Compiled, Vec<String>> {
    let mut cx = Ctx { strat: s, slots: vec![], sources: vec![], errors: vec![], max_offset: 1, exprs: vec![] };
    for (id, def) in &s.indicators {
        if PRICE_FIELDS.contains(&id.as_str()) || id.contains(['.', '[', ']', ' ', '+', '-', '*', '/']) || id.is_empty()
        {
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
        check_distance(d, "exit.stop_loss", s, true, &mut errs);
    }
    if let Some(d) = &x.trailing_stop {
        check_distance(d, "exit.trailing_stop", s, false, &mut errs);
    }
    if let Some(t) = &x.take_profit {
        let level = t.above.is_some() || t.below.is_some();
        let set =
            [t.percent.is_some(), t.atr.is_some(), t.risk_multiple.is_some(), level].iter().filter(|b| **b).count();
        if set != 1 {
            errs.push(
                "exit.take_profit: use exactly one of `percent`, `atr`, `risk_multiple` or price levels (`above`/`below`)"
                    .into(),
            );
        }
        if level && s.entry.long.is_some() && t.above.is_none() {
            errs.push(
                "exit.take_profit: long entries need `above`, the target level (e.g. \"swings.resistance\")".into(),
            );
        }
        if level && s.entry.short.is_some() && t.below.is_none() {
            errs.push("exit.take_profit: short entries need `below`, the target level".into());
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
    let r = &s.risk;
    for (name, v) in [
        ("max_drawdown_percent", r.max_drawdown_percent),
        ("daily_loss_percent", r.daily_loss_percent),
        ("monthly_loss_percent", r.monthly_loss_percent),
    ] {
        if v.is_some_and(|v| !(v > 0.0 && v <= 100.0)) {
            errs.push(format!("risk.{name}: must be between 0 and 100"));
        }
    }
    if r.max_trades_per_day == Some(0) || r.pause_after_losses == Some(0) {
        errs.push("risk: counts must be at least 1".into());
    }
    if r.pause_bars.is_some() && r.pause_after_losses.is_none() {
        errs.push("risk.pause_bars: only applies together with pause_after_losses".into());
    }
    cx.errors.extend(errs);

    let stop_atr = x.stop_loss.as_ref().and_then(|d| cx.atr_ref(d.atr, &d.indicator, "exit.stop_loss"));
    let trail_atr = x.trailing_stop.as_ref().and_then(|d| cx.atr_ref(d.atr, &d.indicator, "exit.trailing_stop"));
    let target_atr = x.take_profit.as_ref().and_then(|t| cx.atr_ref(t.atr, &t.indicator, "exit.take_profit"));
    let mut level = |v: Option<&String>, at: &str| v.and_then(|t| cx.operand(&Operand::Ref(t.clone()), at));
    let stop_below = level(x.stop_loss.as_ref().and_then(|d| d.below.as_ref()), "exit.stop_loss.below");
    let stop_above = level(x.stop_loss.as_ref().and_then(|d| d.above.as_ref()), "exit.stop_loss.above");
    let target_above = level(x.take_profit.as_ref().and_then(|t| t.above.as_ref()), "exit.take_profit.above");
    let target_below = level(x.take_profit.as_ref().and_then(|t| t.below.as_ref()), "exit.take_profit.below");

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
        exprs: cx.exprs,
        stop_below,
        stop_above,
        target_above,
        target_below,
    })
}

// ---------------------------------------------------------------------------
// Evaluation against the history of values.
// ---------------------------------------------------------------------------

/// Per-series history, one value per bar (NaN while an indicator warms up).
pub struct History {
    pub series: Vec<Vec<f64>>,
    exprs: Vec<Lin>,
}

impl History {
    pub fn new(n: usize) -> Self {
        History { series: vec![Vec::new(); n], exprs: vec![] }
    }

    pub fn len(&self) -> usize {
        self.series.first().map_or(0, Vec::len)
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    fn value(&self, series: usize, back: usize) -> Option<f64> {
        let s = &self.series[series];
        let x = s[s.len().checked_sub(1 + back)?];
        x.is_finite().then_some(x)
    }

    fn at(&self, v: Val, back: usize) -> Option<f64> {
        match v {
            Val::Const(c) => Some(c),
            Val::Series { series, offset } => self.value(series, offset + back),
            Val::Expr(i) => {
                let e = &self.exprs[i];
                let mut sum = e.constant;
                for &(k, series, offset) in &e.terms {
                    sum += k * self.value(series, offset + back)?;
                }
                Some(sum)
            }
        }
    }

    /// Current value of a compiled value (for price-level stops and targets).
    pub fn now(&self, v: Val) -> Option<f64> {
        self.at(v, 0)
    }

    pub fn latest(&self, series: usize) -> Option<f64> {
        let x = *self.series[series].last()?;
        x.is_finite().then_some(x)
    }
}

impl Compiled {
    /// An empty history sized for this strategy.
    pub fn history(&self) -> History {
        History { series: vec![Vec::new(); self.sources.len()], exprs: self.exprs.clone() }
    }

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
                    6 => c.hlc3(),
                    7 => (c.close - c.open).abs(),
                    8 => c.high - c.low,
                    9 => c.high - c.open.max(c.close),
                    _ => c.open.min(c.close) - c.low,
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
                Op::Eq => cmp(0).is_some_and(|(l, r)| (l - r).abs() <= 1e-9 * l.abs().max(r.abs()).max(1.0)),
                Op::Ne => cmp(0).is_some_and(|(l, r)| (l - r).abs() > 1e-9 * l.abs().max(r.abs()).max(1.0)),
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
    fn expressions_and_candle_fields() {
        // A hammer written by hand: long lower wick, small body, close in the top third.
        let s = Strategy::from_json(
            r#"{ "name": "x",
                 "entry": { "long": { "all": [
                     { "left": "lower_wick", "op": ">=", "right": "2 * body" },
                     { "left": "close", "op": ">", "right": "low + 0.66 * range" },
                     { "left": "range", "op": ">", "right": "range[1] - 0.5" } ] } },
                 "exit": { "max_bars": 1 } }"#,
        )
        .unwrap();
        let mut c = compile(&s).unwrap();
        assert_eq!(c.exprs.len(), 3);
        let mut h = c.history();
        let bar =
            |o: f64, hi: f64, l: f64, cl: f64| Candle { ts: 0, open: o, high: hi, low: l, close: cl, volume: 1.0 };
        c.push(&bar(10.0, 10.5, 9.5, 10.0), &mut h);
        c.push(&bar(10.0, 10.4, 7.0, 10.3), &mut h); // body 0.3, lower wick 3.0, range 3.4
        assert!(eval(c.entry_long.as_ref().unwrap(), &h));
        c.push(&bar(10.0, 12.0, 9.9, 11.9), &mut h); // big bullish body, tiny lower wick
        assert!(!eval(c.entry_long.as_ref().unwrap(), &h));
    }

    #[test]
    fn expression_errors_are_clear() {
        let bad = |right: &str| {
            let j = format!(
                r#"{{ "name": "x", "entry": {{ "long": {{ "left": "close", "op": ">", "right": "{right}" }} }}, "exit": {{ "max_bars": 1 }} }}"#
            );
            Strategy::from_json(&j).unwrap().validate().join(" ")
        };
        assert!(bad("close * high").contains("multiplied or divided by numbers"));
        assert!(bad("(high + low) / 2").contains("parentheses"));
        assert!(bad("close / 0").contains("division by zero"));
        assert!(bad("clsoe + 1").contains("unknown value `clsoe`"));
    }

    #[test]
    fn crosses_and_lookbacks() {
        let s = Strategy::from_json(
            r#"{ "name": "x", "entry": { "long": { "left": "close", "op": "crosses_above", "right": "close[1]" } }, "exit": { "max_bars": 1 } }"#,
        )
        .unwrap();
        let mut c = compile(&s).unwrap();
        let mut h = c.history();
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
