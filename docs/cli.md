# Command reference

`candlerail <command> --help` shows the same information in the terminal.

## serve

Starts the web app.

```text
candlerail serve [--listen 127.0.0.1:8787] [--ui-dir ui] [--cache-dir DIR] [--binance-api URL]
```

| Flag | Default | |
|---|---|---|
| `--listen` | `127.0.0.1:8787` | Address to bind. The app has no authentication; keep it on localhost. |
| `--ui-dir` | built in | Serve `index.html` from this folder, for editing the UI without rebuilding |
| `--cache-dir` | `~/.candlerail/cache` | Where downloaded candles are stored |
| `--binance-api` | `https://data-api.binance.vision` | Binance-compatible REST endpoint for candles |

## backtest

```text
candlerail backtest <strategy> [--symbol S] [--interval I] [--from DATE] [--to DATE]
                    [--csv FILE] [--capital N] [--json FILE]
```

`<strategy>` is a path to a strategy file or the name of a template.

| Flag | Default | |
|---|---|---|
| `--symbol` | strategy's `market.symbol`, else `BTCUSDT` | Binance spot symbol |
| `--interval` | strategy's `market.interval`, else `1h` | `1m 5m 15m 30m 1h 4h 1d 1w` |
| `--from`, `--to` | depends on the interval, up to now | UTC dates: `2024-01-31`, `2024-01-31 14:00`, or Unix time |
| `--csv` | none | Use candles from a file instead of downloading |
| `--capital` | `10000` | Starting balance |
| `--json` | none | Write the full report: metrics, every trade, the equity curve, indicator values |

Default periods: 7 days for `1m`, 30 for `5m`, 60 for `15m`, 120 for `30m`, a
year for `1h`, two for `4h`, five for `1d`, ten for `1w`.

## explain, check

```text
candlerail explain <strategy>     # plain English, template notes, and problems
candlerail check <strategy>       # problems only; exit code 1 if there are any
```

Both accept pasted AI output with code fences, comments and trailing commas.

## templates, new

```text
candlerail templates                            # list built-in strategies
candlerail new <file> [--from ema-crossover]    # copy a template to a new file
```

`new` refuses to overwrite an existing file.

## indicators, prompt

```text
candlerail indicators     # every indicator: settings, defaults, outputs
candlerail prompt         # instructions for an AI assistant (append your description)
```

## fetch

```text
candlerail fetch --symbol ETHUSDT --interval 4h --from 2023-01-01
```

Downloads candles into the cache without running anything, which is useful
before working offline.

## Exit codes

`0` on success, `1` on any error (invalid strategy, bad arguments, network
failure). Errors go to stderr, results to stdout.
