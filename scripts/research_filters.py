#!/usr/bin/env python3
"""Builds research/results/context-filters.json: what each location and trend
filter did to a candle pattern's results, for the published guide.

Intraday: the Study 1 pin-bar rule on BTC, ETH, SOL and XRP at 15 minutes over
the study's 90 days, run three ways with `candlerail backtest`: the candle
anywhere, only at a swing level, and at a swing level with the 8-hour trend.

Weekly: the Study 4 weekly engulfing rows (long only, 2 ATR stop, held up to
four weeks, normal costs) read from research/results/weekly-patterns.json, by
entry and BTC filter.

    python3 scripts/research_filters.py [path/to/candlerail]
"""

import json
import os
import subprocess
import sys
import tempfile
from datetime import datetime, timezone

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
CLI = sys.argv[1] if len(sys.argv) > 1 else os.path.join(ROOT, "target", "release", "candlerail")
SYMBOLS = ["BTCUSDT", "ETHUSDT", "SOLUSDT", "XRPUSDT"]
FROM, TO = "2026-06-29", "2026-09-27"


def summary(rs):
    wins = sum(1 for r in rs if r > 0)
    return {"trades": len(rs), "wins": wins, "avg_r": round(sum(rs) / len(rs), 3) if rs else None, "sum_r": round(sum(rs), 2)}


def intraday():
    base = json.load(open(os.path.join(ROOT, "research", "pin-level-survivor.json")))
    longs, shorts = base["entry"]["long"]["all"], base["entry"]["short"]["all"]
    # Conditions in the file: pattern, low near the level, close beyond it, trend.
    steps = [
        ("The candle anywhere", "A hammer or shooting star, wherever it forms.", 1),
        ("At a swing level", "Its wick touches the latest swing low (or high) within 0.3 ATR and it closes back beyond the level.", 3),
        ("At a level, with the trend", "Also only in the direction of the 8-hour EMA: longs above it, shorts below.", 4),
    ]
    out = []
    for label, detail, n in steps:
        strategy = json.loads(json.dumps(base))
        strategy["entry"] = {"long": {"all": longs[:n]}, "short": {"all": shorts[:n]}}
        rs = []
        with tempfile.TemporaryDirectory() as tmp:
            path = os.path.join(tmp, "s.json")
            json.dump(strategy, open(path, "w"))
            for sym in SYMBOLS:
                report = os.path.join(tmp, f"{sym}.json")
                subprocess.run([CLI, "backtest", path, "--symbol", sym, "--interval", "15m", "--from", FROM, "--to", TO, "--json", report],
                               check=True, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
                rs += [t["r_multiple"] for t in json.load(open(report))["trades"] if t["r_multiple"] is not None]
        out.append({"label": label, "detail": detail, **summary(rs)})
        print(label, out[-1])
    return out


def weekly():
    d = json.load(open(os.path.join(ROOT, "research", "results", "weekly-patterns.json")))
    fi = [f["id"] for f in d["study"]["families"]].index("w-engulf")
    want = {"side": "long only", "stop": "2 ATR", "target": "hold up to 4 weeks"}
    out = []
    for r in d["rows"]:
        v = d["variants"][fi][r[1]] if r[0] == fi else None
        if v is None or r[3] != 0 or any(v[k] != x for k, x in want.items()):
            continue
        before, after = r[6], r[7]
        trades, wins, sum_r = before[0] + after[0], before[1] + after[1], before[2] + after[2]
        out.append({"entry": v["entry"], "btc": v["btc"], "trades": trades, "wins": wins, "avg_r": round(sum_r / trades, 3),
                    "after_trades": after[0], "after_avg_r": round(after[2] / after[3], 3) if after[3] else None})
    return out


def main():
    doc = {
        "generated": datetime.now(timezone.utc).strftime("%Y-%m-%d"),
        "intraday": {"symbols": SYMBOLS, "interval": "15m", "from": FROM, "to": TO, "costs": "futures taker", "steps": intraday()},
        "weekly": {"coins": 43, "costs": "spot taker", "rows": weekly()},
    }
    with open(os.path.join(ROOT, "research", "results", "context-filters.json"), "w") as f:
        json.dump(doc, f, indent=1)


if __name__ == "__main__":
    main()
