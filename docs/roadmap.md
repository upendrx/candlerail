# Roadmap

What's done is in the [changelog](../CHANGELOG.md). Next, roughly in order:

## Paper trading on live candles
- Run a strategy on live candles, with the same broker rules as backtests.
- Persistent paper accounts, open positions and history in the app.

## Alerts
- "Notify me instead of trading": desktop notifications, Telegram, Discord,
  email and webhooks when a strategy's entry or exit fires.
- Alerts on single conditions ("BTC 4h RSI below 30") without a full strategy.

## Better research
- Parameter sweeps and walk-forward testing.
- Several markets per backtest, portfolio results.
- Funding rates for perpetual futures.

## More ways in
- Desktop app (installer for Windows, macOS, Linux).
- Python bindings and Python strategies.
- Import of simple TradingView Pine Script strategies.
- More data sources (OKX, Kraken, Alpaca for US stocks), sharing live-data
  adapters with [tickrail](https://github.com/upendrx/tickrail).
- More indicators: Ichimoku, Parabolic SAR, pivots, volume profile.
