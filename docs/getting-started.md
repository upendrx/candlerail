# Getting started

## Install

The [quick-start page](https://upendrx.github.io/candlerail/) walks through
this for Windows, macOS and Linux.

**One line**, downloading the latest release:

```bash
# macOS and Linux
curl -fsSL https://raw.githubusercontent.com/upendrx/candlerail/main/scripts/install.sh | sh
```

```powershell
# Windows (PowerShell)
powershell -ExecutionPolicy Bypass -c "irm https://raw.githubusercontent.com/upendrx/candlerail/main/scripts/install.ps1 | iex"
```

The scripts put `candlerail` in `~/.local/bin` or `%LOCALAPPDATA%\candlerail`.
Set `CANDLERAIL_INSTALL_DIR` to choose another folder, or
`CANDLERAIL_VERSION=v0.2.0` for a specific release.

**From source**, with Rust 1.88 or newer ([rustup.rs](https://rustup.rs)):

```bash
git clone https://github.com/upendrx/candlerail
cd candlerail
cargo build --release
```

The binary is `target/release/candlerail` (`candlerail.exe` on Windows). It
runs on Windows, macOS and Linux, is about 5 MB, and uses around 20 MB of memory. Prebuilt
binaries are attached to each GitHub release.

## Open the app

```bash
./target/release/candlerail serve
```

Then open <http://127.0.0.1:8787>. Everything runs on your computer. The only
network traffic is downloading candles from Binance's public API, and they're
cached in `~/.candlerail/cache` after the first time.

1. **Strategies:** pick a template and press *Backtest*.
2. **Results:** read the warnings first, then the drawdown, then the return.
3. **Build:** open a template in the builder and change it. The plain-English
   summary on the right always says exactly what the strategy will do.
4. **Ask AI:** describe a strategy in words, copy the prompt into any chat
   assistant, and paste its reply back.
5. **Price action:** learn candles, levels and patterns, and open the
   worked example in the builder.
6. **Money management:** size positions, compare risk rules, and apply loss
   limits to your strategy.
7. **Community:** load strategies others shared and re-run them to check
   their results.

## From the terminal

```bash
candlerail templates                                  # the built-in strategies
candlerail backtest ema-crossover                     # run one as-is
candlerail new mine.json --from rsi-dip-uptrend       # copy it to edit
candlerail explain mine.json                          # plain English + checks
candlerail backtest mine.json --symbol SOLUSDT --interval 4h --from 2024-01-01
candlerail backtest mine.json --csv my-candles.csv    # any market, from a file
candlerail indicators                                 # everything rules can use
candlerail prompt > prompt.txt                        # instructions for an AI
```

`--json report.json` on `backtest` writes every trade and the equity curve.

## Your own data

Any CSV with time, open, high, low, close and (optionally) volume columns
works: exports from TradingView, brokers, Yahoo Finance and most data vendors.
Times can be dates (`2024-01-31`, `2024-01-31 14:00`, UTC) or Unix seconds or
milliseconds. In the app, use *upload a CSV* under *Market and test period*.
`examples/btcusdt-1h-sample.csv` shows the format.
