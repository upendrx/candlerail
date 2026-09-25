# HTTP API

`candlerail serve` exposes the API the web app is built on. It's JSON over
HTTP on localhost, with no authentication. Use it to drive candlerail from
scripts, notebooks or other front ends.

## GET /api/catalog

Everything a strategy can reference.

```json
{
  "indicators": [
    { "kind": "rsi", "name": "Relative strength index", "category": "momentum",
      "description": "...", "overlay": false, "outputs": ["value"],
      "params": [{ "name": "period", "default": 14, "min": 2, "integer": true, "description": "Number of bars" }] }
  ],
  "price_fields": ["open", "high", "low", "close", "volume", "hl2", "hlc3"],
  "ops": [">", "<", ">=", "<=", "crosses_above", "crosses_below", "rising", "falling"],
  "intervals": ["1m", "5m", "15m", "30m", "1h", "4h", "1d", "1w"]
}
```

## GET /api/templates

`[{ "id": "ema-crossover", "strategy": { ...strategy file... } }, ...]`

## POST /api/check

Parses and validates a strategy, and explains it.

```json
{ "strategy": { ...strategy file... } }
```

`strategy` may also be a **string**, such as an AI reply with code fences or
prose around the JSON.

```json
{
  "ok": true,
  "errors": [],
  "explanation": ["Buy when RSI(14) crosses below 30 ...", "Exit: ...", "..."],
  "strategy": { ...normalised strategy... }
}
```

`ok` is `false` only when the text can't be read as a strategy at all.
Validation problems come back in `errors` with `ok: true`.

## POST /api/backtest

```json
{
  "strategy": { ... },
  "symbol": "ETHUSDT",
  "interval": "4h",
  "from": "2024-01-01",
  "to": "2025-01-01",
  "capital": 10000,
  "csv": "time,open,high,low,close,volume\n..."
}
```

Everything except `strategy` is optional. Defaults come from the strategy's
`market` section and the interval. With `csv`, candles are read from that text
instead of Binance.

The response is `{ "ok": true, "symbol", "interval", "report", "candles" }`.
`report` has:

| Field | |
|---|---|
| `metrics` | return, buy-and-hold, max drawdown, trades, win rate, profit factor, expectancy, Sharpe, fees, liquidations, ... |
| `in_sample`, `out_of_sample` | return, trades, win rate and profit factor for the first 70% and last 30% of the period |
| `trades` | every trade: side, entry/exit time and price, quantity, margin, PnL, return on margin, fees, bars, exit reason |
| `equity` | `{ ts, equity }` at every candle close |
| `indicators` | every indicator output, one value per candle (`null` during warm-up), with an `overlay` flag for charting |
| `warnings` | plain-language cautions about the result |

On failure: `{ "ok": false, "errors": ["..."] }` with status 400 (bad input),
502 (data source unreachable) or 500.

`metrics` also has `avg_r` (average result in R) and `kelly_risk_pct`
(the Kelly risk fraction, once there are ten or more trades with a stop).
Each trade has `r_multiple`. `risk` reports what the strategy's money
management did: `halted_at` (when the drawdown limit stopped trading),
`blocked_entries` (count by rule) and `forced_exits`.

## POST /api/share

Takes the same body as `/api/backtest`, plus optional `author` and `notes`.
Runs the test and returns `{ "ok": true, "share": { ... } }`, a
[share file](sharing.md) with the strategy and its result.

## GET /api/community

The strategies in the community gallery: `[{ "id", "share" }]`.

## GET /api/prompt, GET /schema.json

The AI instructions as plain text, and the JSON Schema for strategy files.
