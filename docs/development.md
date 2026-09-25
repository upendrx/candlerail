# Development guide

## Prerequisites

- Rust 1.88 or newer (`rustup update stable`), with `rustfmt` and `clippy`.
- Nothing else. The UI is plain HTML and JavaScript with no build step, and
  the chart library is vendored.

## Build and run

```bash
cargo build --release
./target/release/candlerail serve --ui-dir ui    # serves ui/index.html from disk
```

With `--ui-dir`, edit `ui/index.html` and reload the browser. Without it, the
UI is the copy compiled into the binary.

`make help` lists shortcuts for the common tasks.

## Tests

```bash
cargo test --workspace
```

| Where | What |
|---|---|
| `crates/core/src/indicators/tests.rs` | Known values, warm-up, parameter validation for every indicator |
| `crates/core/src/spec.rs` | Parsing, error messages, rule evaluation (crosses, lookbacks), tolerant AI-text parsing |
| `crates/core/src/broker.rs` | Fill rules: stop-before-target, gap fills, liquidation, trailing stops, risk sizing |
| `crates/core/src/backtest.rs` | Next-open fills, costs, end-to-end runs |
| `crates/core/src/explain.rs` | Plain-English output |
| `crates/data/src/csv.rs` | CSV layouts from different sources |
| `apps/cli/src/templates.rs` | Every template validates |
| `apps/cli/tests/cli.rs` | The binary end to end, offline, using `examples/btcusdt-1h-sample.csv` |

Tests never touch the network. Anything that changes how fills, stops or
liquidation work needs a test in `broker.rs` that pins down the new behaviour.

## Code style

- `cargo fmt` (120 columns, see `rustfmt.toml`) and
  `cargo clippy --workspace --all-targets -- -D warnings` must be clean. CI
  enforces both.
- `candlerail-core` stays free of I/O: no network, no files, no clocks. That's
  what keeps it testable and reusable.
- Errors meant for users are complete sentences that say how to fix the
  problem, e.g. `sizing: risk_percent needs exit.stop_loss (risk is measured to the stop)`.
- Comments explain *why*. The code says what.

## Where to make common changes

| Change | Files |
|---|---|
| New indicator | `crates/core/src/indicators/{trend,momentum,volatility,volume}.rs`, `indicators/mod.rs` (`CATALOG`, `build`), `schema/strategy.schema.json` |
| New rule operator | `crates/core/src/spec.rs` (`Op`, `eval`), `explain.rs`, the schema, `apps/cli/src/prompt.rs`, `ui/index.html` (`OPS`) |
| New exit or sizing type | `spec.rs`, `broker.rs`, `backtest.rs` (`rules_for`), `explain.rs`, the schema, the builder |
| New metric or warning | `crates/core/src/metrics.rs`, `ui/index.html` (results), `apps/cli/src/report.rs` |
| New data source | a module in `crates/data/src/`, wiring in `apps/cli/src/main.rs` and `server.rs` |
| New template | `templates/<id>.json`, `apps/cli/src/templates.rs` |
| New API endpoint | `apps/cli/src/server.rs`, `docs/api.md` |

## Debugging a backtest

- `candlerail backtest my.json --json out.json` writes every trade, the
  equity curve and every indicator value, so you can check a signal bar by bar.
- `candlerail explain my.json` shows how the rules were understood.
- Indicator values are `null` during warm-up. If a strategy never trades,
  check that its longest indicator has warmed up within the test period.

## Releases

1. Update `CHANGELOG.md`: move `Unreleased` items under the new version.
2. Bump `version` in the root `Cargo.toml`.
3. Merge to `main`, then tag: `git tag -a v0.2.0 -m "v0.2.0" && git push origin v0.2.0`.
4. The release workflow builds binaries for Linux (x86_64, ARM64), macOS
   (Apple Silicon, Intel) and Windows, and attaches them to a GitHub release.

Versioning follows [Semantic Versioning](https://semver.org/). Until 1.0, a
minor version may change the strategy file format; the changelog calls that out.
