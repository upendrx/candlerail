# How backtests work

## Order of events on every candle

1. Orders decided at the previous close fill at **this candle's open**.
2. Stops, targets and liquidation are checked against this candle's high and low.
3. The candle closes. Indicators update, and rules are checked, which may queue
   an entry or exit for the next open.
4. The account is marked to market at the close.

Nothing uses information from the future: a signal at a candle's close can't
fill at that close, because in real life you'd only know the close after it
happened.

## Fills

| Order | Fills at | Pays |
|---|---|---|
| Entry, exit rule, time exit | next open | fee + slippage |
| Stop-loss, trailing stop | the stop price, or the open if the market gapped past it | fee + slippage |
| Take-profit | the target, or the (better) open on a gap | fee |
| Liquidation | the liquidation price | the whole position margin |
| End of test | last close | fee |

**Both stop and target inside one candle:** the stop counts. Candles don't say
which came first, so the test assumes the worse one.

## Leverage and liquidation

Positions use **isolated margin**, as crypto exchanges do. With leverage L, a
long's liquidation price is about `entry × (1 − 1/L + maintenance margin)`: at
10x, roughly 9.5% below entry. If a candle's low reaches it, the trade loses its
whole margin. The rest of the account isn't touched. Funding payments on
perpetual futures aren't modelled yet.

## Metrics

| Metric | Meaning |
|---|---|
| Return | Final balance vs. starting balance |
| Buy and hold | Buying at the first open and holding, without leverage or fees |
| Max drawdown | The worst fall from a peak in the account balance |
| Win rate | Share of trades with positive PnL after fees |
| Profit factor | Total won ÷ total lost. Above 1 made money |
| Expectancy | Average PnL per trade |
| Sharpe | Mean per-candle return ÷ its standard deviation, annualised |
| Time in market | Share of candles with a position open |

## The overfitting check

The period is split at 70%. The results page shows the return, win rate and
profit factor of trades that closed in each part. A strategy that does well in
the first part and badly in the second has probably been tuned to the past, and
the report says so.

## Known limits

- Candle data only: no order book, no queue position. Limit orders at a price
  are assumed to fill if the candle trades through it.
- One position at a time, one market per backtest.
- No funding rates, borrow fees or exchange downtime.
- Results depend on data quality. Binance candles are good, and CSVs from
  elsewhere are only as good as their source.
