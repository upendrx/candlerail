# Contributing to candlerail

Thanks for helping. New indicators, strategy templates, data sources, docs
and bug reports are all welcome, and you don't need to be a Rust expert to
add a template.

## Setup

```bash
git clone https://github.com/upendrx/candlerail && cd candlerail
cargo build --release
cargo test --workspace
./target/release/candlerail serve --ui-dir ui     # edit ui/index.html and reload
```

The [development guide](docs/development.md) covers the code layout, tests,
code style and where to make common changes.

## Before a pull request

```bash
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

## Easy first contributions

- **A template:** a JSON file in `templates/` with a full `about` section (how
  it works, when it works best, when it fails), registered in
  `apps/cli/src/templates.rs`. Include a backtest result in the PR description,
  good or bad. Honest templates are more useful than impressive ones.
- **An indicator:** see [architecture](docs/architecture.md#adding-an-indicator).
  Include a test against values from a reference implementation.
- **Docs:** especially the [strategy guide](docs/strategy-guide.md).

## Rules for the backtester

Changes to `crates/core/src/broker.rs` or `backtest.rs` must keep the
simulation conservative: no fills at prices the strategy couldn't have had,
stops before targets within a candle, fees on every fill. Add a test that
shows the behaviour.

## Branches

`main` is stable and releases are tagged there. Work goes into `develop`
through pull requests from `feature/<name>`, `fix/<name>` or `docs/<name>`
branches.

## License

Contributions are dual-licensed under MIT or Apache-2.0, like the project,
unless you say otherwise.

## Conduct

Everyone taking part follows the [code of conduct](CODE_OF_CONDUCT.md).
