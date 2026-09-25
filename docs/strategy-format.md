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
| `risk` | no | Account-level money-management limits; see [Money management rules](#money-management-rules) |
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
| `==` `!=` | equal / not equal; mostly for flags such as `pa.hammer == 1` or `sw.structure == -1` |
| `crosses_above` | left was at or below right on the previous bar, and is above it now |
| `crosses_below` | the reverse |
| `rising` | left is higher than it was `right` bars ago (`right` is a whole number) |
| `falling` | left is lower than it was `right` bars ago |

**Groups** combine rules and can nest: `{ "all": [...] }` (AND),
`{ "any": [...] }` (OR), `{ "not": <condition> }`.

## Values

- A **number**: `30`, `0.5`.
- A **price field**: `open`, `high`, `low`, `close`, `volume`, `hl2`
  (bar midpoint), `hlc3` (typical price), and the candle's shape: `body`
  (`|close − open|`), `range` (`high − low`), `upper_wick`, `lower_wick`.
- An **indicator id**: `"rsi"` means that indicator's first output.
- An **indicator output**: `"macd.signal"`, `"bb.lower"`, `"adx.plus_di"`.
  `candlerail indicators` lists the outputs of each.
- Any of those **n bars ago**: `"close[1]"`, `"channel.upper[1]"`.
- A **formula**: numbers times values, added or subtracted:
  `"2 * body"`, `"low - 0.5 * range"`, `"1.003 * sw.support"`,
  `"close - close[5]"`. Values can be multiplied or divided by numbers but
  not by each other, and parentheses aren't supported: write
  `"0.5 * high + 0.5 * low"` rather than `"(high + low) / 2"`. Every value
  in a formula can have its own `[n]`.

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

### Stops and targets at price levels

Instead of a distance, a stop or target can sit at a **price level**, any
value or formula, read when the entry signal fires:

```json
"stop_loss":   { "below": "low - 0.1 * range", "above": "high + 0.1 * range" },
"take_profit": { "above": "sw.resistance", "below": "sw.support" }
```

For a stop, `below` is used by longs and `above` by shorts; for a target
it's the other way round. A strategy that trades a side must give that
side's level. If the market opens beyond the stop, or already past the
target, the trade is skipped rather than entered at a loss. Levels work
with `risk_percent` sizing and `risk_multiple` targets: the risk is the
distance from the entry to the level. Trailing stops only take `percent`
or `atr`.

## Sizing

| `type` | `value` means |
|---|---|
| `risk_percent` | Size each trade so that hitting the stop loses this % of the account (needs a `stop_loss`) |
| `percent_equity` | Use this % of the account as margin; the position is that × leverage |
| `fixed` | This many units of the asset (e.g. 0.01 BTC) |

`risk_percent` is the safest starting point: 1% means a run of ten losing
trades costs about 10% of the account.

## Money management rules

The optional `risk` section applies limits to the whole account, the way a
trading desk limits a trader:

```json
"risk": {
  "daily_loss_percent": 3,
  "monthly_loss_percent": 6,
  "max_drawdown_percent": 20,
  "max_trades_per_day": 4,
  "pause_after_losses": 3,
  "pause_bars": 24
}
```

| Field | Effect |
|---|---|
| `daily_loss_percent` | Once the account is this % below where the UTC day started, close open positions and take no entries until the next day |
| `monthly_loss_percent` | The same, for the calendar month |
| `max_drawdown_percent` | Once the account is this % below its peak, close everything and stop trading for the rest of the test |
| `max_trades_per_day` | Ignore entry signals after this many entries in a UTC day |
| `pause_after_losses` | After this many losing trades in a row, skip entries for `pause_bars` candles |

Positions closed by these rules exit at the next open with reason
`risk_limit`. The report counts every blocked entry by rule. See
[Money management](money-management.md) for how to choose the numbers.

## Price-action indicators

These describe raw price rather than smoothing it. The
[price action guide](price-action.md) shows how to combine them.

| `type` | Settings | Outputs |
|---|---|---|
| `patterns` | `wick_ratio` (2), `doji_percent` (10) | `bullish_engulfing`, `bearish_engulfing`, `hammer`, `shooting_star`, `doji`, `inside_bar`, `outside_bar`, `morning_star`, `evening_star`, `three_white_soldiers`, `three_black_crows`, `bullish_marubozu`, `bearish_marubozu`: 1 on the candle that completes the pattern, else 0 |
| `swings` | `left` (3), `right` (3) | `resistance`, `support` (latest confirmed swing high and low), `prev_resistance`, `prev_support`, `structure` (1 higher highs and lows, −1 lower, 0 mixed) |
| `period` | `minutes` (1440) | `open`, `high`, `low` of the current higher-timeframe period so far; `prev_open`, `prev_high`, `prev_low`, `prev_close` of the last completed one. 60, 240, 1440 (UTC day), 10080 (week from Monday) or 43200 (calendar month) |
| `opening_range` | `minutes` (15), `session_start` (0) | `high`, `low` of the first `minutes` of each session, and `ready` (1 once complete). `session_start` is minutes after 00:00 UTC, e.g. 810 for 13:30 UTC |
| `volume_avg` | `period` (20) | `average` volume of the previous candles, and `relative`: this candle's volume ÷ that average |

A swing is only confirmed `right` candles after it happens, and `period`
levels only change when a period completes, so none of these look ahead.
