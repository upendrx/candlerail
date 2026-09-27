# Research: intraday reversals on major coins

Can well-known day-trading setups, mostly candle reversals at support and
resistance, make a daily profit on BTC, ETH, SOL and XRP once real trading
costs are paid? This page records a study that tried to find out, and how to
run it yourself. The app's **Research** tab shows the same results with an
explorer for every variant.

![The Research tab](images/research.png)

**Short answer: no setup held up.** About 26,000 backtests found one variant
that passed selection and a held-out month, and it lost money on five other
coins and on the three months before.

## The setups

Fifteen setups, each traded long and short, plus an opening-range breakout
as a trend-following reference:

| Setup | Long side (short mirrors it) |
|---|---|
| Engulfing at swing level | Bullish engulfing within half an ATR of the latest swing low |
| Pin bar at swing level | Hammer that holds the latest swing low |
| Liquidity sweep and reclaim | Low runs below the swing low, close back above it, long lower wick |
| Small candle, then big breakout candle | Inside bar, then a candle 1.5× its range closing above it |
| Morning star at swing level | Morning star whose middle candle sits at the swing low |
| Outside-bar reversal | After three falling closes, an outside bar closing above the previous high |
| Previous hour / 4-hour / day rejection | Dip under the previous period's low, close back above with a long lower wick |
| Reversal far from VWAP | Two ATRs below the day's VWAP, then an engulfing or hammer |
| Bollinger band reclaim | Pierce the lower band, close back inside, up |
| Engulfing out of an RSI extreme | RSI below 30 on the previous candle, then a bullish engulfing |
| Pullback in a trend | 20 EMA above 50, dip to the 20, engulfing or hammer |
| Big, small, big back | Large red candle, small candle, strong green candle above the small one |
| Change of character | In a structure of lower highs and lows, a close above the last swing high |

Every setup was crossed with a stop (beyond the signal candle, ATR multiples
or a fixed percentage), a target of 1, 1.5 or 2 times the risk, and either
no trend filter or trading only with an 8-hour EMA. Every trade risked 1% of
the account (10x isolated leverage, only so the position fits), with a 3%
daily loss limit and a one-hour pause after four losses in a row. Trades
closed after 4 hours (round 1) or 8 hours (round 2).

## Method

- **Data:** Binance candles for the 90 days to 27 September 2026.
- **Fills:** signal at a candle's close, market order at the next open, stop
  before target when a candle touches both.
- **Costs:** *futures taker* 0.05% fee and 0.01% slippage per fill; *spot
  taker* 0.1% and 0.02%. Futures funding isn't included.
- **Selection, fixed before looking:** a variant is *selected* if it trades
  often enough, averages at least +0.05R over the first 60 days, and makes
  money there on at least 3 of the 4 coins. It *survives* if it then averages
  above 0R over the last 30 days and makes money on at least 3 coins.
- **Trend:** each trade is tagged by the coin's change over the 24 hours
  before it: above +1.5% is an uptrend, below −1.5% a downtrend, else sideways.

## Results

### Round 1: 1, 5 and 15-minute candles

465 variants × 4 coins × 3 timeframes × 2 cost levels: **11,160 backtests**.
Of the 1,395 variant-timeframe combinations under futures costs, 7 averaged
above zero in the selection period and **none was selected**.

The reason is costs. Fees alone, as a multiple of each trade's risk:

| Timeframe | Median fees per trade, futures | Spot |
|---|---|---|
| 1m | 1.38R | 2.40R |
| 5m | 0.50R | 0.96R |
| 15m | 0.33R | 0.67R |

A 0.1% stop with a 0.2% target pays 1.2R per round trip on futures, and needs
to win 73% of the time just to break even. Before costs, the best setups
averaged only +0.1R to +0.3R.

### Round 2: wider stops

Informed only by the cost result: stops of 0.8%, 1.2%, 1.5 ATR, 2 ATR or
beyond the signal candle, 5 to 60-minute candles, holds of up to 8 hours, and
at least 1.5 trades a day counted across the four coins together. **14,880
backtests.** Under futures costs, 73 of 1,860 combinations were positive in
the selection period, **10 were selected and 1 survived**:

> **Pin bar at swing level**, 15-minute candles, 1.2% stop, 2R target, only
> with the 8-hour trend: +0.24R a trade in the selection period, +0.11R in
> the held-out month, profitable there on 3 of 4 coins, about 1.9 trades a
> day across the four.

Its trades by market trend: +0.59R in uptrends, +0.15R sideways, −0.09R in
downtrends.

### Confirmation on data it never saw

| Test | Trades | Average | Coins profitable |
|---|---|---|---|
| Held-out month (round 2) | 66 | +0.11R | 3 of 4 |
| Five other coins (BNB, DOGE, ADA, AVAX, LINK), same 90 days | 225 | **−0.06R** | 1 of 5 |
| The same four coins, the 90 days before | 182 | **−0.14R** | 1 of 4 |

At 1% risk that is roughly +0.47% a day in the period it was chosen on,
+0.21% in the held-out month, and −0.15% and −0.28% a day on unseen data.
The survivor was luck.

## What it means

- **Don't scalp at taker fees.** No candle pattern here had an edge close to
  what 1- and 5-minute stops pay in fees.
- **Candle reversals at support and resistance showed no reliable edge** on
  these coins in these months, even with wide stops.
- **A good backtest isn't evidence until it repeats** on data it wasn't
  chosen on. A study that tests thousands of variants will always find some
  that look good.
- Worth testing next: entries with limit orders at the maker fee, fewer and
  longer trades where costs are a small share of the risk, and repeating
  this study as new months arrive.

## Running a study

```bash
candlerail study research/intraday-reversals-2.json --digest my-run.json
```

This downloads any candles it needs (months of 1-minute data take a few
minutes the first time), runs every variant in parallel, and prints the
selected variants. `-o FILE` writes the full result; `--digest FILE` writes
the compact form the app reads. In the app, **Re-run on the latest data** does
the same for the selected study.

### The study file

```json
{
  "name": "My study",
  "symbols": ["BTCUSDT", "ETHUSDT"],
  "intervals": ["5m", "15m"],
  "days": 90,
  "offset_days": 0,
  "split": 0.6667,
  "capital": 10000,
  "costs": [{ "label": "futures taker", "fee_bps": 5, "slippage_bps": 1 }],
  "selection": { "portfolio": true, "min_trades_per_day": 1.5, "max_trades_per_day": 20, "min_avg_r": 0.05, "min_markets": 2 },
  "grid": {
    "rr": [1, 2],
    "stop": [{ "label": "1.2%", "v": { "percent": 1.2 } }, { "label": "2 ATR", "v": { "atr": 2, "indicator": "atr" } }]
  },
  "families": [{
    "id": "pin", "name": "Pin bar at support", "idea": "A hammer that holds the swing low.",
    "strategy": {
      "name": "pin",
      "indicators": { "sw": { "type": "swings" }, "pa": { "type": "patterns" }, "atr": { "type": "atr" } },
      "entry": { "long": { "all": [
        { "left": "pa.hammer", "op": "==", "right": 1 },
        { "left": "low", "op": "<=", "right": "sw.support + 0.3 * atr" }
      ] } },
      "exit": { "stop_loss": "{{stop.v}}", "take_profit": { "risk_multiple": "{{rr}}" }, "max_bars": "{{hours:8}}" },
      "sizing": { "type": "risk_percent", "value": 1 },
      "leverage": 10
    }
  }]
}
```

A family's `strategy` is an ordinary strategy file with placeholders:

| Placeholder | Becomes |
|---|---|
| `"{{rr}}"` | the grid value of `rr`: a number, string or whole object |
| `"{{stop.v}}"` | the field `v` of an object value; objects can carry a `label` for display |
| `"{{hours:8}}"` | the number of candles in 8 hours at the timeframe being tested |
| `"sw.support + {{k}} * atr"` | a placeholder inside text is replaced by its value |

Only settings a template uses multiply its variants. A family can add its
own `grid`. `offset_days` ends the period that many days ago, for testing on
older data; `split: 0` counts every trade as unseen, for confirmation runs.
The files for the studies above are in [`research/`](../research).
