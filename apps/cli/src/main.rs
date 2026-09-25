//! `candlerail`: backtest candle strategies described in a JSON file.
//!
//! ```text
//! candlerail serve                                  web app at http://127.0.0.1:8787
//! candlerail backtest my.json --symbol ETHUSDT      backtest from the terminal
//! candlerail explain my.json                        the strategy in plain English
//! candlerail templates / new FILE --from TEMPLATE   start from a built-in strategy
//! candlerail indicators                             what you can use in rules
//! candlerail prompt                                 instructions for an AI assistant
//! candlerail fetch --symbol BTCUSDT --interval 1h   download candles into the cache
//! ```

mod prompt;
mod report;
mod server;
mod templates;

use anyhow::{Context, Result, bail};
use candlerail_core::{BacktestConfig, Candle, Interval, Strategy, explain, indicators, time};
use candlerail_data::{Query, binance};
use clap::{Args, Parser, Subcommand};
use std::path::{Path, PathBuf};

#[derive(Parser)]
#[command(name = "candlerail", version, about = "Build and backtest candle strategies from a JSON file")]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Start the local web app.
    Serve {
        #[arg(long, default_value = "127.0.0.1:8787")]
        listen: String,
        /// Serve the UI from this folder instead of the built-in copy.
        #[arg(long)]
        ui_dir: Option<PathBuf>,
        #[command(flatten)]
        data: DataArgs,
    },
    /// Backtest a strategy file (or a template name) and print the report.
    Backtest {
        /// Path to a strategy JSON file, or the name of a built-in template.
        strategy: String,
        #[command(flatten)]
        market: MarketArgs,
        #[command(flatten)]
        data: DataArgs,
        /// Write the full report (trades, equity curve) as JSON.
        #[arg(long)]
        json: Option<PathBuf>,
    },
    /// Describe a strategy in plain English and check it for mistakes.
    Explain { strategy: String },
    /// Only check a strategy for mistakes.
    Check { strategy: String },
    /// Download candles into the cache.
    Fetch {
        #[command(flatten)]
        market: MarketArgs,
        #[command(flatten)]
        data: DataArgs,
    },
    /// List the built-in strategy templates.
    Templates,
    /// Copy a template to a new strategy file you can edit.
    New {
        /// File to create, e.g. my-strategy.json
        file: PathBuf,
        #[arg(long, default_value = "ema-crossover")]
        from: String,
    },
    /// List indicators, their parameters and outputs.
    Indicators,
    /// Print instructions to paste into an AI assistant so it writes valid strategies.
    Prompt,
}

#[derive(Args, Clone)]
struct MarketArgs {
    /// Binance symbol, e.g. BTCUSDT. Defaults to the strategy's market.symbol.
    #[arg(long)]
    symbol: Option<String>,
    /// Candle interval: 1m 5m 15m 30m 1h 4h 1d 1w. Defaults to the strategy's market.interval.
    #[arg(long)]
    interval: Option<String>,
    /// Start date (UTC), e.g. 2024-01-01. Defaults depend on the interval.
    #[arg(long)]
    from: Option<String>,
    /// End date (UTC), exclusive. Defaults to now.
    #[arg(long)]
    to: Option<String>,
    /// Use candles from a CSV file instead of downloading.
    #[arg(long)]
    csv: Option<PathBuf>,
    /// Starting account balance.
    #[arg(long, default_value_t = 10_000.0)]
    capital: f64,
}

#[derive(Args, Clone)]
struct DataArgs {
    /// Where downloaded candles are cached (default ~/.candlerail/cache).
    #[arg(long)]
    cache_dir: Option<PathBuf>,
    #[arg(long, default_value = binance::DEFAULT_API)]
    binance_api: String,
}

impl DataArgs {
    fn cache(&self) -> PathBuf {
        self.cache_dir.clone().unwrap_or_else(default_cache)
    }
}

pub fn default_cache() -> PathBuf {
    let home = std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE")).map(PathBuf::from);
    home.unwrap_or_else(|| PathBuf::from(".")).join(".candlerail").join("cache")
}

/// How far back to go by default: enough bars to mean something, few enough
/// that the first download is quick.
pub fn default_span(i: Interval) -> i64 {
    let day = 86_400_000;
    day * match i {
        Interval::M1 => 7,
        Interval::M5 => 30,
        Interval::M15 => 60,
        Interval::M30 => 120,
        Interval::H1 => 365,
        Interval::H4 => 730,
        Interval::D1 => 1_825,
        Interval::W1 => 3_650,
    }
}

pub fn now_ms() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as i64).unwrap_or(0)
}

fn load_strategy(arg: &str) -> Result<Strategy> {
    let text = if Path::new(arg).exists() {
        std::fs::read_to_string(arg).with_context(|| format!("reading {arg}"))?
    } else if let Some(t) = templates::get(arg) {
        t.to_string()
    } else {
        bail!("`{arg}` is neither a file nor a template (run `candlerail templates`)");
    };
    Strategy::from_ai_text(&text).map_err(anyhow::Error::msg)
}

fn ensure_valid(s: &Strategy) -> Result<()> {
    let errs = s.validate();
    if errs.is_empty() {
        return Ok(());
    }
    bail!("the strategy has {} problem(s):\n  - {}", errs.len(), errs.join("\n  - "));
}

struct Loaded {
    symbol: String,
    interval: Interval,
    candles: Vec<Candle>,
}

fn load_candles(s: Option<&Strategy>, m: &MarketArgs, d: &DataArgs) -> Result<Loaded> {
    let interval: Interval = match m.interval.as_deref() {
        Some(i) => i.parse().map_err(anyhow::Error::msg)?,
        None => s.and_then(|s| s.market.interval).unwrap_or(Interval::H1),
    };
    let date = |v: &Option<String>, what: &str| -> Result<Option<i64>> {
        v.as_deref().map(|t| time::parse(t).with_context(|| format!("bad --{what} date `{t}`"))).transpose()
    };
    if let Some(path) = &m.csv {
        let candles = candlerail_data::csv::load(path)?;
        let (from, to) = (date(&m.from, "from")?.unwrap_or(i64::MIN), date(&m.to, "to")?.unwrap_or(i64::MAX));
        let symbol = path.file_stem().map_or("CSV".into(), |f| f.to_string_lossy().into_owned());
        return Ok(Loaded { symbol, interval, candles: candlerail_data::tidy(candles, from, to) });
    }
    let symbol =
        m.symbol.clone().or_else(|| s.and_then(|s| s.market.symbol.clone())).unwrap_or_else(|| "BTCUSDT".into());
    let to = date(&m.to, "to")?.unwrap_or_else(now_ms);
    let from = date(&m.from, "from")?.unwrap_or(to - default_span(interval));
    let q = Query { symbol: symbol.to_uppercase(), interval, from, to };
    eprint!("loading {} {interval} candles ...", q.symbol);
    let candles = binance::load(&d.binance_api, &d.cache(), &q, |n| {
        eprint!("\rloading {} {interval} candles ... {n}   ", q.symbol)
    })?;
    eprintln!(
        "\r{} {interval}: {} candles, {} to {}            ",
        q.symbol,
        candles.len(),
        time::format(from),
        time::format(to)
    );
    Ok(Loaded { symbol: q.symbol, interval, candles })
}

fn main() -> Result<()> {
    match Cli::parse().cmd {
        Cmd::Serve { listen, ui_dir, data } => server::serve(&listen, ui_dir, data.cache(), data.binance_api),
        Cmd::Backtest { strategy, market, data, json } => {
            let s = load_strategy(&strategy)?;
            ensure_valid(&s)?;
            let l = load_candles(Some(&s), &market, &data)?;
            let cfg = BacktestConfig { capital: market.capital, interval: l.interval, ..BacktestConfig::default() };
            let r = candlerail_core::run(&s, &l.candles, &cfg).map_err(|e| anyhow::anyhow!(e.join("\n")))?;
            report::print(&r, &l.symbol, l.interval);
            if let Some(path) = json {
                std::fs::write(&path, serde_json::to_string_pretty(&r)?)?;
                println!("\nfull report written to {}", path.display());
            }
            Ok(())
        }
        Cmd::Explain { strategy } => {
            let s = load_strategy(&strategy)?;
            println!("{}\n", s.name);
            for p in explain::explain(&s) {
                println!("{p}\n");
            }
            if let Some(a) = &s.about {
                println!(
                    "How it works: {}\n\nWorks best: {}\n\nFails when: {}\n",
                    a.how_it_works, a.works_best, a.fails_when
                );
            }
            ensure_valid(&s)?;
            println!("No problems found.");
            Ok(())
        }
        Cmd::Check { strategy } => {
            let s = load_strategy(&strategy)?;
            ensure_valid(&s)?;
            println!("{strategy}: ok");
            Ok(())
        }
        Cmd::Fetch { market, data } => {
            let l = load_candles(None, &market, &data)?;
            println!(
                "cached {} candles for {} {} in {}",
                l.candles.len(),
                l.symbol,
                l.interval,
                data.cache().display()
            );
            Ok(())
        }
        Cmd::Templates => {
            for (id, text) in templates::ALL {
                let s = Strategy::from_json(text).map_err(anyhow::Error::msg)?;
                let cat = s.about.as_ref().map_or("", |a| a.category.as_str());
                println!("{id:<24} {cat:<18} {}", s.description);
            }
            println!("\ncandlerail new my-strategy.json --from <name>    copy one to edit");
            println!("candlerail backtest <name>                        run one as-is");
            Ok(())
        }
        Cmd::New { file, from } => {
            let t =
                templates::get(&from).with_context(|| format!("no template `{from}` (run `candlerail templates`)"))?;
            if file.exists() {
                bail!("{} already exists", file.display());
            }
            std::fs::write(&file, t)?;
            println!(
                "created {} from {from}. Edit it, then run: candlerail backtest {}",
                file.display(),
                file.display()
            );
            Ok(())
        }
        Cmd::Indicators => {
            for i in indicators::CATALOG {
                let params: Vec<String> = i.params.iter().map(|p| format!("{}={}", p.name, p.default)).collect();
                println!("{:<11} {:<30} outputs: {}", i.kind, i.name, i.outputs.join(", "));
                println!("{:<11} params: {}", "", if params.is_empty() { "none".into() } else { params.join(", ") });
                println!("{:<11} {}\n", "", i.description);
            }
            Ok(())
        }
        Cmd::Prompt => {
            print!("{}", prompt::build());
            Ok(())
        }
    }
}
