# Changelog

All notable changes are recorded here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and the project uses
[Semantic Versioning](https://semver.org/).

## [Unreleased]

### Added
- Price action: `patterns` (13 candlestick patterns), `swings` (support,
  resistance and market structure), `period` (previous hour, day, week and
  month levels on any timeframe), `opening_range` and `volume_avg`.
- Candle fields `body`, `range`, `upper_wick`, `lower_wick`; `==` and `!=`
  operators; formulas such as `low - 0.5 * range` anywhere a value goes.
- Stops and targets at price levels (`below` / `above`).
- Account-level money management (`risk`): daily and monthly loss limits,
  maximum drawdown, trades per day, pause after a losing streak.
- R-multiple per trade, average R and a Kelly estimate in the metrics.
- Share files (`candlerail share`, `POST /api/share`) and a community
  gallery (`GET /api/community`) with recorded, re-runnable results.
- Eight price-action templates, from a 1-minute scalp to a monthly breakout.
- Web app: Start, Price action, Money management and Community tabs;
  formulas, price-level exits and risk rules in the builder; sharing and
  re-run verification.
- Install scripts for macOS, Linux and Windows, and a quick-start site.
- Strategy file format (JSON) with a published JSON Schema.
- 14 streaming indicators: SMA, EMA, RSI, MACD, Bollinger Bands, ATR,
  stochastic, session VWAP, SuperTrend, ADX, OBV, Donchian, rate of change,
  Keltner channel.
- Rule engine: comparisons, crossovers, rising/falling, lookbacks, AND/OR/NOT groups.
- Backtester with next-open fills, stop-first intrabar handling, fees,
  slippage, leverage with isolated-margin liquidation, stop-loss, take-profit,
  trailing stop, time exits and three sizing modes.
- Metrics with first-70%/last-30% split and plain-language warnings.
- Plain-English explanation of any strategy.
- Binance candles with a local cache; CSV import for any market.
- Ten documented strategy templates.
- `candlerail` CLI and a local web app with a no-code builder, AI prompt
  helper, charts and trade lists.
- HTTP API (`/api/catalog`, `/api/templates`, `/api/check`, `/api/backtest`,
  `/api/prompt`, `/schema.json`).
- User and developer documentation, and end-to-end CLI tests that run offline.

### Changed
- Release archives are named without the version (`candlerail-<target>`) so
  the latest release has stable download links; Windows builds are `.zip`.
