# Architecture

```text
apps/cli            candlerail binary: CLI commands and the local web server
crates/core         candles, indicators, strategy format, broker, backtest, metrics, explain (no I/O)
crates/data         candle sources: Binance klines with a cache, CSV files
ui/                 the web app (plain HTML and JS, compiled into the binary)
templates/          built-in strategies (compiled into the binary)
schema/             JSON Schema for strategy files
community/          shared strategies with recorded results (the Community gallery)
site/               the quick-start web page, published to GitHub Pages
scripts/            one-line installers for macOS, Linux and Windows
```

`candlerail-core` has no network or file access. It's a pure function from
(strategy, candles, settings) to a report, which makes it easy to test and to
reuse in other front ends: a desktop app, Python bindings, a live runner.

## A backtest, step by step

1. `spec::compile` resolves every reference in the rules (`macd.signal`,
   `close[1]`) to a series index once, and reports all problems together.
2. `backtest::run` walks the candles. For each one it fills pending orders in
   the `Broker`, checks exits against the range, feeds the candle to every
   indicator, records the values the rules need, and evaluates the rules.
3. `metrics` summarises trades and the equity curve and writes the warnings.

## Adding an indicator

1. Implement `Indicator` (`update(&Candle)`, `get(output)`) in the matching file
   under `crates/core/src/indicators/`. Return `None` until warmed up.
2. Add an `IndicatorInfo` entry to `CATALOG` (name, parameters with defaults,
   outputs, whether it draws on the price chart) and an arm in `build`.
3. Add a test with known values to `indicators/tests.rs`.

The catalog drives the builder, the AI prompt, the schema list and the
`indicators` command, so the new indicator appears everywhere. Also add its
`type` to the enum in `schema/strategy.schema.json`.

## Adding a template

Put a JSON file in `templates/` with a complete `about` section, add it to
`apps/cli/src/templates.rs`, and run `cargo test`. A test checks that every
template is valid.
