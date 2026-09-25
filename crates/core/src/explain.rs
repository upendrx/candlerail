//! Turns a strategy file into plain English, so people can check that what an
//! AI (or they themselves) wrote is what they meant, before risking anything.

use crate::indicators;
use crate::spec::{Condition, Distance, Op, Operand, SizingType, Strategy, Target};
use std::collections::BTreeMap;

struct Names {
    labels: BTreeMap<String, (String, &'static indicators::IndicatorInfo, Vec<f64>)>,
}

fn article(word: &str) -> &'static str {
    if word.starts_with(['a', 'e', 'i', 'o', 'u']) { "an" } else { "a" }
}

fn period_name(minutes: f64) -> String {
    match minutes as u64 {
        60 => "hour".into(),
        240 => "4-hour period".into(),
        1440 => "day".into(),
        10_080 => "week".into(),
        43_200 => "month".into(),
        m if m % 1440 == 0 => format!("{}-day period", m / 1440),
        m if m % 60 == 0 => format!("{}-hour period", m / 60),
        m => format!("{m}-minute period"),
    }
}

/// Natural names for price-action outputs, which read badly as `LABEL output`.
fn price_action_name(kind: &str, out: &str, params: &[f64]) -> Option<String> {
    Some(match (kind, out) {
        ("patterns", o) => {
            let n = o.replace('_', " ");
            format!("{} {n}", article(&n))
        }
        ("swings", "resistance") => "the latest swing high (resistance)".into(),
        ("swings", "support") => "the latest swing low (support)".into(),
        ("swings", "prev_resistance") => "the previous swing high".into(),
        ("swings", "prev_support") => "the previous swing low".into(),
        ("swings", "structure") => "market structure".into(),
        ("period", o) => {
            let p = period_name(params.first().copied().unwrap_or(1440.0));
            match o {
                "open" => format!("this {p}'s open"),
                "high" => format!("this {p}'s high so far"),
                "low" => format!("this {p}'s low so far"),
                other => format!("the previous {p}'s {}", other.trim_start_matches("prev_")),
            }
        }
        ("opening_range", o) => {
            let m = params.first().copied().unwrap_or(15.0);
            match o {
                "ready" => format!("the {m:.0}-minute opening range is complete"),
                side => format!("the {m:.0}-minute opening-range {side}"),
            }
        }
        ("volume_avg", "relative") => {
            format!("volume relative to the last {:.0} candles", params.first().copied().unwrap_or(20.0))
        }
        ("volume_avg", _) => format!("average volume over {:.0} candles", params.first().copied().unwrap_or(20.0)),
        _ => return None,
    })
}

impl Names {
    fn new(s: &Strategy) -> Self {
        let mut labels = BTreeMap::new();
        for (id, def) in &s.indicators {
            let Some(kind) = def.get("type").and_then(|v| v.as_str()) else { continue };
            let Some(info) = indicators::info(kind) else { continue };
            let params = indicators::resolve_params(info, def).unwrap_or_default();
            labels.insert(id.clone(), (indicators::label(info, &params), info, params));
        }
        Names { labels }
    }

    fn value(&self, text: &str) -> String {
        if crate::spec::is_expression_text(text) {
            return self.expression(text);
        }
        let (name, back) = match text.split_once('[') {
            Some((n, rest)) => (n.trim(), rest.trim_end_matches(']').trim().parse::<usize>().unwrap_or(0)),
            None => (text.trim(), 0),
        };
        let base = match name {
            "open" => "the open".to_string(),
            "high" => "the high".to_string(),
            "low" => "the low".to_string(),
            "close" => "the close".to_string(),
            "volume" => "volume".to_string(),
            "hl2" => "the bar midpoint".to_string(),
            "hlc3" => "the typical price".to_string(),
            "body" => "the candle body".to_string(),
            "range" => "the candle range".to_string(),
            "upper_wick" => "the upper wick".to_string(),
            "lower_wick" => "the lower wick".to_string(),
            _ => {
                let (id, out) = name.split_once('.').map_or((name, None), |(a, b)| (a, Some(b)));
                match self.labels.get(id) {
                    Some((label, info, params)) => {
                        let o = out.unwrap_or(info.outputs[0]);
                        if let Some(n) = price_action_name(info.kind, o, params) {
                            n
                        } else if out.is_some() && info.outputs.len() > 1 && o != info.kind && o != "value" {
                            format!("{label} {}", o.replace('_', " "))
                        } else {
                            label.clone()
                        }
                    }
                    None => format!("`{name}`"),
                }
            }
        };
        match back {
            0 => base,
            1 => format!("{base} one candle ago"),
            n => format!("{base} {n} candles ago"),
        }
    }

    /// `2 * body[1] + 0.5` → `2 × the candle body one bar ago + 0.5`
    fn expression(&self, text: &str) -> String {
        let mut out = String::new();
        let mut word = String::new();
        let flush = |w: &mut String, out: &mut String, names: &Names| {
            if w.is_empty() {
                return;
            }
            let t = w.trim();
            if t.parse::<f64>().is_ok() {
                out.push_str(t);
            } else {
                out.push_str(&names.value(t));
            }
            w.clear();
        };
        for c in text.chars() {
            match c {
                '+' | '-' | '*' | '/' => {
                    flush(&mut word, &mut out, self);
                    out.push_str(match c {
                        '+' => " + ",
                        '-' => " − ",
                        '*' => " × ",
                        _ => " ÷ ",
                    });
                }
                ' ' => flush(&mut word, &mut out, self),
                _ => word.push(c),
            }
        }
        flush(&mut word, &mut out, self);
        out.trim().replace("  ", " ")
    }

    fn operand(&self, o: &Operand) -> String {
        match o {
            Operand::Number(n) => trim(*n),
            Operand::Ref(s) => s.parse::<f64>().map(trim).unwrap_or_else(|_| self.value(s)),
        }
    }

    fn cond(&self, c: &Condition) -> String {
        match c {
            Condition::All { all } => self.join(all, " and "),
            Condition::Any { any } => self.join(any, " or "),
            Condition::Not { not } => format!("it is not the case that {}", self.cond(not)),
            Condition::Rule(r) => {
                let l = self.operand(&r.left);
                let rt = self.operand(&r.right);
                match r.op {
                    Op::Gt => format!("{l} is above {rt}"),
                    Op::Lt => format!("{l} is below {rt}"),
                    Op::Ge => format!("{l} is at or above {rt}"),
                    Op::Le => format!("{l} is at or below {rt}"),
                    Op::CrossesAbove => format!("{l} crosses above {rt}"),
                    Op::CrossesBelow => format!("{l} crosses below {rt}"),
                    Op::Eq if rt == "1" && self.flag(&r.left) == Some("patterns") => format!("{l} forms"),
                    Op::Eq if rt == "0" && self.flag(&r.left) == Some("patterns") => format!("{l} does not form"),
                    Op::Eq if rt == "1" && self.flag(&r.left) == Some("ready") => l,
                    Op::Eq if rt == "1" && self.flag(&r.left) == Some("structure") => {
                        "market structure is up (higher highs and higher lows)".into()
                    }
                    Op::Eq if rt == "-1" && self.flag(&r.left) == Some("structure") => {
                        "market structure is down (lower highs and lower lows)".into()
                    }
                    Op::Eq => format!("{l} equals {rt}"),
                    Op::Ne => format!("{l} is not {rt}"),
                    Op::Rising => format!("{l} is higher than {rt} bars ago"),
                    Op::Falling => format!("{l} is lower than {rt} bars ago"),
                }
            }
        }
    }

    /// Flag-like outputs read better as sentences: "a hammer forms" rather than "equals 1".
    fn flag(&self, o: &Operand) -> Option<&'static str> {
        let Operand::Ref(t) = o else { return None };
        let id = t.split(['.', '[']).next().unwrap_or("");
        let out = t.split_once('.').map(|(_, o)| o.split('[').next().unwrap_or(o));
        let (_, info, _) = self.labels.get(id)?;
        match (info.kind, out) {
            ("patterns", _) => Some("patterns"),
            ("opening_range", Some("ready")) => Some("ready"),
            ("swings", Some("structure")) => Some("structure"),
            _ => None,
        }
    }

    fn join(&self, parts: &[Condition], sep: &str) -> String {
        let v: Vec<String> = parts
            .iter()
            .map(|c| match c {
                Condition::Rule(_) => self.cond(c),
                _ => format!("({})", self.cond(c)),
            })
            .collect();
        v.join(sep)
    }
}

fn trim(n: f64) -> String {
    if n.fract() == 0.0 { format!("{n:.0}") } else { format!("{n}") }
}

fn distance(d: &Distance, names: &Names) -> String {
    if d.is_level() {
        let mut v = vec![];
        if let Some(b) = &d.below {
            v.push(format!("{} for longs", names.value(b)));
        }
        if let Some(a) = &d.above {
            v.push(format!("{} for shorts", names.value(a)));
        }
        return format!("{} (read when the signal fires)", v.join(" and "));
    }
    match (d.percent, d.atr) {
        (Some(p), _) => format!("{}% from the entry price", trim(p)),
        (_, Some(a)) => {
            format!(
                "{} × {} from the entry price",
                trim(a),
                d.indicator.as_deref().map_or("ATR".into(), |i| names.value(i))
            )
        }
        _ => "an unspecified distance".into(),
    }
}

fn target(t: &Target, names: &Names) -> String {
    if t.above.is_some() || t.below.is_some() {
        let mut v = vec![];
        if let Some(a) = &t.above {
            v.push(format!("{} for longs", names.value(a)));
        }
        if let Some(b) = &t.below {
            v.push(format!("{} for shorts", names.value(b)));
        }
        return v.join(" and ");
    }
    match (t.percent, t.atr, t.risk_multiple) {
        (Some(p), _, _) => format!("{}% from the entry price", trim(p)),
        (_, Some(a), _) => {
            format!(
                "{} × {} from the entry price",
                trim(a),
                t.indicator.as_deref().map_or("ATR".into(), |i| names.value(i))
            )
        }
        (_, _, Some(m)) => format!("{} times the stop distance (a 1:{} risk/reward)", trim(m), trim(m)),
        _ => "an unspecified level".into(),
    }
}

/// A few short paragraphs describing the whole strategy.
pub fn explain(s: &Strategy) -> Vec<String> {
    let n = Names::new(s);
    let mut out = vec![];
    let when = |c: &Condition| format!("when {}", n.cond(c));
    match (&s.entry.long, &s.entry.short) {
        (Some(l), Some(sh)) => out.push(format!(
            "Buy (go long) {}. Sell short {}. Signals are checked when each bar closes and filled at the next bar's open.",
            when(l),
            when(sh)
        )),
        (Some(l), None) => {
            out.push(format!("Buy {}. Signals are checked when each bar closes and filled at the next bar's open.", when(l)))
        }
        (None, Some(sh)) => out.push(format!(
            "Sell short {}. Signals are checked when each bar closes and filled at the next bar's open.",
            when(sh)
        )),
        (None, None) => out.push("This strategy has no entry rule, so it never trades.".into()),
    }

    let x = &s.exit;
    let mut exits = vec![];
    if let Some(c) = &x.long {
        exits.push(format!("close a long {}", when(c)));
    }
    if let Some(c) = &x.short {
        exits.push(format!("close a short {}", when(c)));
    }
    if let Some(d) = &x.stop_loss {
        exits.push(format!("cut the loss at {}", distance(d, &n)));
    }
    if let Some(t) = &x.take_profit {
        exits.push(format!("take profit at {}", target(t, &n)));
    }
    if let Some(d) = &x.trailing_stop {
        exits.push(format!(
            "trail a stop {} behind the best price since entry",
            distance(d, &n).replace(" from the entry price", "")
        ));
    }
    if let Some(b) = x.max_bars {
        exits.push(format!("give up after {b} bars"));
    }
    if !exits.is_empty() {
        let last = exits.pop().unwrap_or_default();
        let text = if exits.is_empty() { last } else { format!("{}, and {last}", exits.join(", ")) };
        out.push(format!("Exit: {text}."));
    }

    let size = match s.sizing.kind {
        SizingType::PercentEquity => format!("Each trade puts up {}% of the account as margin", trim(s.sizing.value)),
        SizingType::RiskPercent => {
            format!("Each trade is sized so that hitting the stop loses {}% of the account", trim(s.sizing.value))
        }
        SizingType::Fixed => format!("Each trade is a fixed {} units", trim(s.sizing.value)),
    };
    if s.leverage > 1.0 {
        out.push(format!(
            "{size}, at {}x leverage. At that leverage a move of about {:.1}% against the position liquidates it.",
            trim(s.leverage),
            100.0 / s.leverage - s.costs.maintenance_margin_percent
        ));
    } else {
        out.push(format!("{size}, without leverage."));
    }
    let r = &s.risk;
    let mut rules = vec![];
    if let Some(x) = r.max_drawdown_percent {
        rules.push(format!("stop trading for good if the account falls {}% from its peak", trim(x)));
    }
    if let Some(x) = r.daily_loss_percent {
        rules.push(format!("stop for the day after losing {}% of the day's starting balance", trim(x)));
    }
    if let Some(x) = r.monthly_loss_percent {
        rules.push(format!("stop for the month after losing {}% of the month's starting balance", trim(x)));
    }
    if let Some(x) = r.max_trades_per_day {
        rules.push(format!("take at most {x} trades a day"));
    }
    if let Some(x) = r.pause_after_losses {
        rules.push(format!("pause for {} candles after {x} losses in a row", r.pause_bars.unwrap_or(24)));
    }
    if !rules.is_empty() {
        out.push(format!(
            "Money management: {}. Open positions are closed when a loss limit is hit.",
            rules.join("; ")
        ));
    }
    out.push(format!(
        "Costs assumed: {}% fee per fill and {}% slippage on market and stop orders.",
        trim(s.costs.fee_bps / 100.0),
        trim(s.costs.slippage_bps / 100.0)
    ));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_naturally() {
        let s = Strategy::from_json(
            r#"{ "name": "t",
                 "indicators": { "rsi": { "type": "rsi" }, "trend": { "type": "sma", "period": 200 }, "m": { "type": "macd" } },
                 "entry": { "long": { "all": [ { "left": "rsi", "op": "crosses_below", "right": 30 },
                                               { "left": "close", "op": ">", "right": "trend" },
                                               { "left": "m.macd", "op": ">", "right": "m.signal[1]" } ] } },
                 "exit": { "stop_loss": { "percent": 2 }, "take_profit": { "risk_multiple": 2 } },
                 "sizing": { "type": "risk_percent", "value": 1 }, "leverage": 5 }"#,
        )
        .unwrap();
        let text = explain(&s).join(" ");
        for needle in [
            "Buy when RSI(14) crosses below 30 and the close is above SMA(200)",
            "MACD(12, 26, 9) signal one candle ago",
            "cut the loss at 2% from the entry price",
            "2 times the stop distance",
            "loses 1% of the account",
            "5x leverage",
            "19.5%",
        ] {
            assert!(text.contains(needle), "missing `{needle}` in: {text}");
        }
    }
}
