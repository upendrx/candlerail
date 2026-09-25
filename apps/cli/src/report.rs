//! Prints a backtest report to the terminal.

use candlerail_core::{Interval, Report, time};

fn money(x: f64) -> String {
    let s = format!("{:.2}", x.abs());
    let (int, frac) = s.split_once('.').unwrap_or((&s, "00"));
    let mut grouped = String::new();
    for (i, ch) in int.chars().enumerate() {
        if i > 0 && (int.len() - i) % 3 == 0 {
            grouped.push(',');
        }
        grouped.push(ch);
    }
    format!("{}{grouped}.{frac}", if x < 0.0 { "-" } else { "" })
}

fn pf(x: Option<f64>) -> String {
    x.map_or("n/a (no losses)".into(), |v| format!("{v:.2}"))
}

pub fn print(r: &Report, symbol: &str, interval: Interval) {
    let m = &r.metrics;
    println!();
    println!(
        "{}  ·  {symbol} {interval}  ·  {} to {}  ·  {} bars",
        r.strategy,
        time::format(r.start_ts),
        time::format(r.end_ts),
        r.bars
    );
    println!("{}", "─".repeat(78));
    println!("  Return            {:>9.2}%     Buy and hold      {:>9.2}%", m.total_return_pct, m.buy_hold_return_pct);
    println!("  Final equity   {:>12}      Max drawdown      {:>9.2}%", money(m.final_equity), m.max_drawdown_pct);
    println!("  Trades            {:>9}      Win rate          {:>9.1}%", m.trades, m.win_rate_pct);
    println!("  Profit factor     {:>9}      Sharpe (annual)   {:>9.2}", pf(m.profit_factor), m.sharpe);
    println!("  Avg win        {:>12}      Avg loss       {:>12}", money(m.avg_win), money(m.avg_loss));
    println!("  Expectancy     {:>12}      Fees paid      {:>12}", money(m.expectancy), money(m.fees_paid));
    println!("  Time in market    {:>9.1}%     Avg bars held     {:>9.1}", m.exposure_pct, m.avg_bars_held);
    println!("  Liquidations      {:>9}      Worst losing run  {:>9}", m.liquidations, m.longest_losing_streak);
    println!();
    println!(
        "  First 70% of data: {:+.2}% over {} trades (PF {})   Last 30%: {:+.2}% over {} trades (PF {})",
        r.in_sample.return_pct,
        r.in_sample.trades,
        pf(r.in_sample.profit_factor),
        r.out_of_sample.return_pct,
        r.out_of_sample.trades,
        pf(r.out_of_sample.profit_factor)
    );
    if !r.warnings.is_empty() {
        println!("\nThings to keep in mind:");
        for w in &r.warnings {
            println!("  ! {w}");
        }
    }
    if !r.trades.is_empty() {
        println!("\nLast trades:");
        println!(
            "  {:<17} {:<6} {:>12} {:>12} {:>11} {:>8}  exit",
            "entry", "side", "entry px", "exit px", "pnl", "bars"
        );
        for t in r.trades.iter().rev().take(10).collect::<Vec<_>>().into_iter().rev() {
            println!(
                "  {:<17} {:<6} {:>12.4} {:>12.4} {:>11} {:>8}  {:?}",
                time::format(t.entry_ts),
                format!("{:?}", t.side).to_lowercase(),
                t.entry_price,
                t.exit_price,
                money(t.pnl),
                t.bars,
                t.reason
            );
        }
    }
    println!("\nPast results don't predict future ones. This is a simulation: no orders were sent anywhere.");
}
