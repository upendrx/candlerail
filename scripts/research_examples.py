#!/usr/bin/env python3
"""Builds research/examples.json: real trades of each study's headline rule on
real candles, for the charts in the published research.

Each example runs `candlerail backtest --json` on one market and keeps a
window of candles with the trades and indicator lines inside it. Candles come
from candlerail's cache, so run the backtests (or `candlerail fetch`) first.

    python3 scripts/research_examples.py [path/to/candlerail]
"""

import csv
import json
import os
import subprocess
import sys
import tempfile
from datetime import datetime, timezone

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
CLI = sys.argv[1] if len(sys.argv) > 1 else os.path.join(ROOT, "target", "release", "candlerail")
CACHE = os.path.join(os.path.expanduser("~"), ".candlerail", "cache")

EXAMPLES = [
    {
        "id": "pin-level",
        "title": "Pin bar at a swing level, BTC, 15 minutes",
        "strategy": "research/pin-level-survivor.json",
        "symbol": "BTCUSDT", "interval": "15m", "from": "2026-08-01",
        "window": ("2026-08-27", "2026-08-30"),
        "lines": [("sw", "support", "Swing support"), ("sw", "resistance", "Swing resistance")],
    },
    {
        "id": "supertrend",
        "title": "SuperTrend (10, 3), ETH, 4 hours, long and short",
        "strategy": "research/supertrend-pick.json",
        "symbol": "ETHUSDT", "interval": "4h", "from": "2025-06-01",
        "window": ("2025-06-01", "2025-10-20"),
        "lines": [("st", "value", "SuperTrend")],
    },
    {
        "id": "weekly-engulf",
        "title": "Weekly bullish engulfing with a daily confirmation, SOL, daily",
        "strategy": "research/weekly-engulf-pick.json",
        "symbol": "SOLUSDT", "interval": "1d", "from": "2023-01-01",
        "window": ("2023-06-01", "2024-04-15"),
        "lines": [],
        "weekly": True,
    },
]


def ms(day):
    return int(datetime.strptime(day, "%Y-%m-%d").replace(tzinfo=timezone.utc).timestamp() * 1000)


def candles(symbol, interval, a, b):
    out = []
    with open(os.path.join(CACHE, f"binance-{symbol}-{interval}.csv")) as f:
        for row in csv.DictReader(f):
            ts = int(row.get("ts") or row.get("time") or row.get("open_time"))
            if a <= ts < b:
                out.append([ts] + [round(float(row[k]), 6) for k in ("open", "high", "low", "close")])
    return out


def weekly(daily):
    """Monday-to-Sunday candles from daily ones."""
    weeks = {}
    for ts, o, h, l, c in daily:
        day = ts // 86_400_000
        key = (day - 4) // 7  # 1970-01-05 was a Monday
        if key not in weeks:
            weeks[key] = [(key * 7 + 4) * 86_400_000, o, h, l, c]
        else:
            w = weeks[key]
            w[2], w[3], w[4] = max(w[2], h), min(w[3], l), c
    return [weeks[k] for k in sorted(weeks)]


def main():
    out = []
    for ex in EXAMPLES:
        with tempfile.NamedTemporaryFile(suffix=".json", delete=False) as tmp:
            path = tmp.name
        subprocess.run(
            [CLI, "backtest", os.path.join(ROOT, ex["strategy"]), "--symbol", ex["symbol"], "--interval", ex["interval"],
             "--from", ex["from"], "--json", path],
            check=True, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL,
        )
        report = json.load(open(path))
        os.unlink(path)
        a, b = ms(ex["window"][0]), ms(ex["window"][1])
        full = candles(ex["symbol"], ex["interval"], ms(ex["from"]), b)
        start = next(i for i, c in enumerate(full) if c[0] >= a)
        window = full[start:]
        lines = []
        for ind, output, label in ex["lines"]:
            series = next(s for s in report["indicators"] if s["id"] == ind and s["output"] == output)
            lines.append({"label": label, "values": [None if v is None else round(v, 6) for v in series["values"][start:start + len(window)]]})
        trades = [
            {"side": t["side"], "entry_ts": t["entry_ts"], "exit_ts": t["exit_ts"], "entry": round(t["entry_price"], 6),
             "exit": round(t["exit_price"], 6), "r": round(t["r_multiple"] or 0, 3), "reason": t["reason"]}
            for t in report["trades"] if a <= t["entry_ts"] < b
        ]
        item = {"id": ex["id"], "title": ex["title"], "symbol": ex["symbol"], "interval": ex["interval"],
                "candles": window, "lines": lines, "trades": trades}
        if ex.get("weekly"):
            item["weekly"] = weekly(window)
        out.append(item)
        print(f"{ex['id']}: {len(window)} candles, {len(trades)} trades")
    with open(os.path.join(ROOT, "research", "examples.json"), "w") as f:
        json.dump({"generated": datetime.now(timezone.utc).strftime("%Y-%m-%d"), "examples": out}, f, separators=(",", ":"))


if __name__ == "__main__":
    main()
