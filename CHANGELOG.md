# Changelog

All notable changes are recorded here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and the project uses
[Semantic Versioning](https://semver.org/).

## [Unreleased]

### Added
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
