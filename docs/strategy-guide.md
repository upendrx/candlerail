# A guide to strategy types

Most strategies are one of a handful of ideas. Knowing which one you're
building tells you what market it needs and how it will fail. Each section
names the template that implements it.

## Trend following

**Idea:** prices that are moving tend to keep moving. Enter after a move has
started and stay in until it ends.

**Typical signals:** a fast moving average crossing a slow one, price above a
long average, SuperTrend or ADX direction.

**What to expect:** a low win rate (30 to 45%) with occasional large winners.
Long losing streaks in sideways markets are normal; that's the cost of being
in the big moves.

**Templates:** `ema-crossover`, `supertrend-long-short`, `adx-trend-strength`.

## Mean reversion

**Idea:** prices that stretch far from normal tend to snap back.

**Typical signals:** RSI or the stochastic at extremes, closes outside the
Bollinger Bands.

**What to expect:** a high win rate with small wins, and occasional big losses
when "stretched" turns into a new trend. A stop-loss and a trend filter
matter more here than anywhere else.

**Templates:** `rsi-dip-uptrend`, `bollinger-reversion`, `stochastic-oversold`.

## Breakout

**Idea:** when price escapes a range, the move often continues.

**Typical signals:** a close above the previous N-bar high (Donchian), or
outside a volatility channel (Keltner, Bollinger).

**What to expect:** many false starts. It works when a few breakouts turn
into trends. Always compare with the *previous* bar's channel.

**Templates:** `donchian-breakout`, `keltner-breakout`.

## Momentum

**Idea:** accelerating price change predicts more of the same in the short term.

**Typical signals:** MACD crossing its signal line, rate of change, RSI
crossing 50.

**Templates:** `macd-momentum`.

## Intraday

**Idea:** trade within the day around session levels like VWAP, and be flat
overnight.

**What to expect:** many trades, so **fees decide everything**. A strategy
that makes 0.1% per trade before costs loses money at a 0.1% fee per fill.
Check the *fees paid* number first.

**Templates:** `vwap-intraday`.

## Building your own

1. **Start from a template** close to your idea, not from a blank page.
2. **Change one thing at a time** and backtest after each change.
3. **Add a filter, not more signals.** A trend filter (price above a 200-bar
   average) often helps a mean-reversion entry more than a second oscillator.
4. **Decide the exit before the entry.** Where does the idea stop being true?
   That's your stop.
5. **Size by risk.** `risk_percent` 0.5 to 1 keeps a losing streak survivable.
6. **Test other markets and periods.** An idea that only works on one coin
   over one year is probably noise.

## Common mistakes

- **Tuning until it looks perfect.** Every extra rule fits the past a little
  better and the future a little worse. The results page compares the first 70%
  and last 30% of the period for this reason.
- **Ignoring fees.** Leave the default costs on unless you know your real fee tier.
- **Too few trades.** Under 30 trades, a good result is often luck.
- **Leverage to fix a weak strategy.** Leverage multiplies the losses as much
  as the gains, and adds liquidation. Make it work at 1x first.
- **Beating nothing.** Compare with buy and hold. In a bull market many
  strategies make money and still lose to doing nothing.
