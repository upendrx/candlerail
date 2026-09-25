# Writing strategies with an AI assistant

candlerail doesn't run code written by an AI. The AI writes a **strategy file**,
which can only use the indicators, rules and exits candlerail understands.
That keeps a mistaken or made-up answer from doing anything unexpected, and
lets candlerail explain the result back to you in plain English before you
run it.

## In the app

1. Open **Ask AI**, and describe the strategy: market, timeframe, entry, exit, risk.
2. Press **Copy prompt** and paste it into any chat assistant, hosted or running
   locally (for example through Ollama or LM Studio).
3. Paste the reply back and press **Check and load**. Code fences, comments and
   extra text around the JSON are fine.
4. Read the summary. If it isn't what you meant, say what's wrong to the
   assistant, or fix it in the builder.

## Elsewhere

- `candlerail prompt` prints the same instructions for use in any tool.
- The web app serves them at `GET /api/prompt`, and the format at `GET /schema.json`.
- `POST /api/check` with `{"strategy": "<pasted text>"}` returns the parsed
  strategy, any problems, and the explanation. That's everything needed to wire
  a model in directly.

## Describing a strategy well

A good description names:

- **The market and timeframe:** "BTCUSDT on the 4 hour chart".
- **The entry:** "when the 20 EMA crosses above the 50 EMA and RSI is above 50".
- **The exit:** "stop 2% below entry, take profit at twice the risk, or close
  when the 20 EMA crosses back below".
- **The risk:** "risk 1% of the account per trade, no leverage".
