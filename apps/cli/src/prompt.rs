//! The instructions given to an AI model so it writes valid strategy files.
//! Generated from the indicator catalog, so it's always in sync with the code.

use candlerail_core::indicators::CATALOG;
use candlerail_core::spec::PRICE_FIELDS;

pub fn build() -> String {
    let mut ind = String::new();
    for i in CATALOG {
        let params: Vec<String> = i.params.iter().map(|p| format!("{}={}", p.name, p.default)).collect();
        ind.push_str(&format!(
            "- \"{}\" ({}): outputs [{}]; params {}. {}\n",
            i.kind,
            i.name,
            i.outputs.join(", "),
            if params.is_empty() { "none".into() } else { params.join(", ") },
            i.description
        ));
    }
    format!(
        r#"You write trading strategies for candlerail as a single JSON object. Reply with the JSON only, no commentary.

FORMAT
{{
  "name": "short name",
  "description": "one sentence",
  "market": {{ "symbol": "BTCUSDT", "interval": "1h" }},          // intervals: 1m 5m 15m 30m 1h 4h 1d 1w
  "indicators": {{ "<id>": {{ "type": "<kind>", <params> }} }},     // ids are your own names, e.g. "rsi", "trend"
  "entry": {{ "long": <condition>, "short": <condition> }},         // either or both
  "exit": {{
    "long": <condition>, "short": <condition>,                      // optional exit rules
    "stop_loss": {{ "percent": 2 }} or {{ "atr": 2, "indicator": "<atr id>" }},
    "take_profit": {{ "percent": 4 }} or {{ "atr": 3, "indicator": "<atr id>" }} or {{ "risk_multiple": 2 }},
    "trailing_stop": {{ "percent": 3 }} or {{ "atr": 2, "indicator": "<atr id>" }},
    "max_bars": 24
  }},
  "sizing": {{ "type": "risk_percent", "value": 1 }},    // or "percent_equity" (margin % of account) or "fixed" (units)
  "leverage": 1,                                         // 1 to 125
  "costs": {{ "fee_bps": 10, "slippage_bps": 2 }}
}}

CONDITIONS
- A rule: {{ "left": <value>, "op": "<op>", "right": <value> }}
- Groups: {{ "all": [ ... ] }} (AND), {{ "any": [ ... ] }} (OR), {{ "not": <condition> }}
- ops: ">", "<", ">=", "<=", "crosses_above", "crosses_below", "rising", "falling"
  ("rising"/"falling": right is a whole number of bars, e.g. {{ "left": "close", "op": "rising", "right": 3 }})
- A value is a number, a price field ({price_fields}), an indicator id (its first output),
  or "<id>.<output>". Add [n] for n bars ago: "close[1]", "bb.upper[2]".

INDICATORS
{ind}
RULES
- Every value you reference must be a price field or an indicator you defined.
- Always include a stop_loss unless the user explicitly says not to.
- "risk_percent" sizing requires a stop_loss. Use 1 unless the user says otherwise.
- "risk_multiple" take profit requires a stop_loss.
- For breakouts compare with the previous bar's channel, e.g. "close" > "channel.upper[1]".
- Keep leverage at 1 unless the user asks for leverage.

EXAMPLE
User: buy bitcoin on the 4 hour chart when RSI drops under 30 while price is above the 200 average, risk 1%, stop 3%, take profit at twice the risk.
{{
  "name": "RSI dip above SMA 200",
  "market": {{ "symbol": "BTCUSDT", "interval": "4h" }},
  "indicators": {{ "rsi": {{ "type": "rsi", "period": 14 }}, "trend": {{ "type": "sma", "period": 200 }} }},
  "entry": {{ "long": {{ "all": [
    {{ "left": "rsi", "op": "crosses_below", "right": 30 }},
    {{ "left": "close", "op": ">", "right": "trend" }} ] }} }},
  "exit": {{ "stop_loss": {{ "percent": 3 }}, "take_profit": {{ "risk_multiple": 2 }} }},
  "sizing": {{ "type": "risk_percent", "value": 1 }}
}}

Now write the strategy the user describes:
"#,
        price_fields = PRICE_FIELDS.join(", "),
    )
}
