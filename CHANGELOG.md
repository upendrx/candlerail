# Changelog

All notable changes are recorded here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and the project uses
[Semantic Versioning](https://semver.org/).

## [Unreleased]

### Added
- Research studies (`candlerail study`, `candlerail_core::study`): expand
  strategy templates over a grid, run every variant across markets,
  timeframes and cost scenarios in parallel, select on the first part of the
  period and judge on the rest, with results by market trend.
- A recorded day-trading study of 15 intraday setups on BTC, ETH, SOL and
  XRP (two rounds and two confirmation runs), and a Research tab to explore
  it, see what fees do to tight stops, and re-run it on the latest data.
- `/api/studies` endpoints.
- Study 2: eleven swing and trend strategies on ten coins over three years,
  re-tested on the three years before, with month-by-month results against
  buying and holding, and a plan page for a small account.
- Funding-rate carry (`candlerail carry`, `candlerail_core::carry`) using
  Binance's public funding history.
- Portfolio research (`candlerail quant`, `candlerail_core::quant`): coin
  rotation, Fear & Greed sentiment (`candlerail_data::sentiment`), time of
  day and weekday, and pairs, on 43 coins including delisted ones.
- Study 3: those four studies, how position size changes returns and
  drawdowns (with simulated years), and combining the trend rule with coin
  rotation, with `GET /api/quant/{id}`.
- Numbered research pages (`research/index.json`, `GET /api/research`,
  `GET /api/carry/{id}`); studies record monthly portfolio results and a
  buy-and-hold benchmark, grid values can hold placeholders, and families
  can be limited to some timeframes.
- Chart Lab: select 1 to 5 candles on a chart to turn their shape into
  rules, see every match and what followed, loosen rules automatically,
  backtest with setup-based stops, and compare across timeframes and coins.
  Mark swings to find matching swing settings, draw levels to use in rules,
  and save setups in the browser.
- `POST /api/candles` and `POST /api/scan`, and `candlerail_core::scan`.
- A refreshed look for the web app and site: new type, softer colours,
  and a matching dark theme.
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
