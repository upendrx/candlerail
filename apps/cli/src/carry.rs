//! Runs the funding-rate carry trade over a basket of coins.

use crate::now_ms;
use anyhow::Result;
use candlerail_core::carry::{self, CarryConfig, CarryResult, Mode};
use candlerail_core::study::Months;
use candlerail_data::funding;
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Coin {
    pub symbol: String,
    pub payments: usize,
    pub always: CarryResult,
    pub filtered: CarryResult,
}

/// A carry run: every coin held all the time, and only while recent funding is
/// positive, plus the two as an equal-weight portfolio.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CarryStudy {
    pub name: String,
    pub description: String,
    pub generated_at: i64,
    pub from: i64,
    pub to: i64,
    pub month0: i64,
    pub config: CarryConfig,
    pub filter_days: u32,
    pub coins: Vec<Coin>,
    pub portfolio_always: Vec<f64>,
    pub portfolio_filtered: Vec<f64>,
}

pub fn run(symbols: &[String], days: u32, offset_days: u32, api: &str, cache: &Path) -> Result<CarryStudy> {
    let to = now_ms() - offset_days as i64 * 86_400_000;
    let from = to - days as i64 * 86_400_000;
    let months = Months::between(from, to);
    let cfg = CarryConfig::default();
    let filter_days = 7;
    let mut coins = vec![];
    for s in symbols {
        eprint!("\rloading {s} funding ...          ");
        let rates = funding::load(api, cache, s, from, to)?;
        coins.push(Coin {
            symbol: s.to_uppercase(),
            payments: rates.len(),
            always: carry::run(&rates, &cfg, Mode::Always, months),
            filtered: carry::run(&rates, &cfg, Mode::PositiveTrailing { days: filter_days }, months),
        });
    }
    eprintln!("\r                                  ");
    let n = coins.len().max(1) as f64;
    let avg = |f: fn(&Coin) -> &CarryResult| -> Vec<f64> {
        (0..months.count)
            .map(|k| coins.iter().map(|c| f(c).monthly.get(k).copied().unwrap_or(0.0)).sum::<f64>() / n)
            .collect()
    };
    let portfolio_always = avg(|c| &c.always);
    let portfolio_filtered = avg(|c| &c.filtered);
    Ok(CarryStudy {
        name: String::new(),
        description: String::new(),
        generated_at: now_ms(),
        from,
        to,
        month0: months.first,
        config: cfg,
        filter_days,
        coins,
        portfolio_always,
        portfolio_filtered,
    })
}

pub fn print(c: &CarryStudy) {
    let yearly = |m: &[f64]| m.iter().sum::<f64>() / (m.len().max(1) as f64 / 12.0);
    println!(
        "{:<10} {:>9} {:>10} {:>10} {:>9} {:>8}",
        "coin", "payments", "always/yr", "filter/yr", "negative", "held"
    );
    for x in &c.coins {
        println!(
            "{:<10} {:>9} {:>9.2}% {:>9.2}% {:>8.0}% {:>7.0}%",
            x.symbol,
            x.payments,
            yearly(&x.always.monthly),
            yearly(&x.filtered.monthly),
            100.0 * x.always.negative_share,
            100.0 * x.filtered.held_share
        );
    }
    println!(
        "{:<10} {:>9} {:>9.2}% {:>9.2}%   (equal-weight portfolio, % of the account a year)",
        "portfolio",
        "",
        yearly(&c.portfolio_always),
        yearly(&c.portfolio_filtered)
    );
}
