# The strategy file

A strategy is one JSON file. The builder, the AI assistant and hand-editing
all produce the same format, so you can move between them freely. The formal
definition is [`schema/strategy.schema.json`](../schema/strategy.schema.json).

```json
{
  "name": "RSI dip in an uptrend",
  "market": { "symbol": "BTCUSDT", "interval": "1h" },
  "indicators": {
    "rsi": { "type": "rsi", "period": 14 },
    "trend": { "type": "sma", "period": 200 }
  },
  "entry": {
    "long": { "all": [
      { "left": "rsi", "op": "crosses_below", "right": 30 },
      { "left": "close", "op": ">", "right": "trend" }
    ] }
  },
  "exit": {
    "long": { "left": "rsi", "op": ">", "right": 55 },
    "stop_loss": { "percent": 3 },
    "max_bars": 48
  },
  "sizing": { "type": "risk_percent", "value": 1 },
  "leverage": 1,
  "costs": { "fee_bps": 10, "slippage_bps": 2 }
}
```

## Fields

| Field | Required | Meaning |
|---|---|---|
| `name` | yes | Display name |
| `description`, `about` | no | Text for people; `about` holds `category`, `timeframe`, `how_it_works`, `works_best`, `fails_when` |
| `market` | no | Default `symbol` and `interval` (`1m 5m 15m 30m 1h 4h 1d 1w`); the app and CLI can override them |
| `indicators` | no | Indicators by id: `"<id>": { "type": "<kind>", <settings> }`. Missing settings use defaults. |
| `entry` | yes | `long` and/or `short` conditions |
| `exit` | see below | Exit rules and protective orders |
| `sizing` | no | Default: 100% of the account as margin |
| `leverage` | no | 1 to 125. Default 1 |
| `costs` | no | `fee_bps` (default 10 = 0.1%), `slippage_bps` (default 2), `maintenance_margin_percent` (default 0.5) |

Every side you enter needs a way out: an exit rule for that side, or a
`stop_loss`, `take_profit`, `trailing_stop` or `max_bars`.

## Conditions

A **rule** compares two values:

```json
{ "left": "close", "op": "crosses_above", "right": "ema50" }
```

| `op` | True when |
|---|---|
| `>` `<` `>=` `<=` | the comparison holds on this bar |
| `crosses_above` | left was at or below right on the previous bar, and is above it now |
| `crosses_below` | the reverse |
| `rising` | left is higher than it was `right` bars ago (`right` is a whole number) |
| `falling` | left is lower than it was `right` bars ago |

**Groups** combine rules and can nest: `{ "all": [...] }` (AND),
`{ "any": [...] }` (OR), `{ "not": <condition> }`.

## Values

- A **number**: `30`, `0.5`.
- A **price field**: `open`, `high`, `low`, `close`, `volume`, `hl2`
  (bar midpoint), `hlc3` (typical price).
- An **indicator id**: `"rsi"` means that indicator's first output.
- An **indicator output**: `"macd.signal"`, `"bb.lower"`, `"adx.plus_di"`.
  `candlerail indicators` lists the outputs of each.
- Any of those **n bars ago**: `"close[1]"`, `"channel.upper[1]"`.

Breakouts almost always want the previous bar's channel: a Donchian channel
includes the current bar, so `close > dc.upper` can never be true, while
`close > dc.upper[1]` is a breakout.

While an indicator warms up (a 200-bar average has no value for its first
199 bars), any rule that uses it is false.

## Exits

```json
"exit": {
  "long": <condition>,                                  // close a long at the next open
  "short": <condition>,
  "stop_loss":    { "percent": 2 }  or { "atr": 2, "indicator": "atr" },
  "take_profit":  { "percent": 4 }  or { "atr": 3, "indicator": "atr" } or { "risk_multiple": 2 },
  "trailing_stop":{ "percent": 3 }  or { "atr": 2, "indicator": "atr" },
  "max_bars": 24
}
```

ATR distances use the ATR value at the bar the entry signal fired.
`risk_multiple: 2` puts the target twice as far from the entry as the stop.

## Sizing

| `type` | `value` means |
|---|---|
| `risk_percent` | Size each trade so that hitting the stop loses this % of the account (needs a `stop_loss`) |
| `percent_equity` | Use this % of the account as margin; the position is that × leverage |
| `fixed` | This many units of the asset (e.g. 0.01 BTC) |

`risk_percent` is the safest starting point: 1% means a run of ten losing
trades costs about 10% of the account.
