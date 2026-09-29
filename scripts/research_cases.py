#!/usr/bin/env python3
"""Builds research/results/cases-*.json: real trades of each study's rule, with
the candles around them and every condition of the rule checked by hand, so the
published pages can show exactly why each one was a trade and where to find it
on a Binance chart.

Trades come from `candlerail backtest --json`; examples are spread evenly
through each rule's trades in date order, never picked by result. Each rule
condition is recomputed here from the raw candles and indicator values, and the
script fails if any example doesn't pass, so the pages can't show a trade the
rule wouldn't have taken.

    python3 scripts/research_cases.py [path/to/candlerail]
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
PER_RULE = 10
DAY = 86_400_000


def utc(ts, date_only=False):
    d = datetime.fromtimestamp(ts / 1000, timezone.utc)
    return d.strftime("%Y-%m-%d" if date_only else "%Y-%m-%d %H:%M")


def ms(day):
    return int(datetime.strptime(day, "%Y-%m-%d").replace(tzinfo=timezone.utc).timestamp() * 1000)


def px(v):
    """A price with sensible precision."""
    a = abs(v)
    d = 2 if a >= 100 else 3 if a >= 10 else 4 if a >= 1 else 5 if a >= 0.1 else 6
    return f"{v:,.{d}f}"


def rnd(v):
    return None if v is None else float(f"{v:.8g}")


def load(sym, interval):
    rows = []
    with open(os.path.join(CACHE, f"binance-{sym}-{interval}.csv")) as f:
        for r in csv.DictReader(f):
            rows.append([int(r["time"]), float(r["open"]), float(r["high"]), float(r["low"]), float(r["close"])])
    return rows


def backtest(strategy, sym, interval, start, end=None):
    with tempfile.TemporaryDirectory() as tmp:
        spath, rpath = os.path.join(tmp, "s.json"), os.path.join(tmp, "r.json")
        json.dump(strategy, open(spath, "w"))
        cmd = [CLI, "backtest", spath, "--symbol", sym, "--interval", interval, "--from", start, "--json", rpath]
        if end:
            cmd += ["--to", end]
        subprocess.run(cmd, check=True, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        return json.load(open(rpath))


def aligned(report, sym, interval):
    """The report's candles and indicator series, index-aligned."""
    rows = [c for c in load(sym, interval) if c[0] >= report["start_ts"]][: report["bars"]]
    assert len(rows) == report["bars"], (sym, interval, len(rows), report["bars"])
    ind = {f"{s['id']}.{s['output']}": s["values"] for s in report["indicators"]}
    return rows, ind


def spread(items, n):
    items = sorted(items, key=lambda x: x["trade"]["entry_ts"])
    if len(items) <= n:
        return items
    return [items[round(i * (len(items) - 1) / (n - 1))] for i in range(n)]


def window(rows, a, b):
    return [[r[0], rnd(r[1]), rnd(r[2]), rnd(r[3]), rnd(r[4])] for r in rows[max(0, a): b]]


def series(ind, key, a, b):
    return [rnd(v) if v is not None else None for v in ind[key][max(0, a): b]]


def check(ok, text):
    return {"ok": bool(ok), "text": text}


def trade_info(t, stop, target=None):
    return {"side": t["side"], "entry_ts": t["entry_ts"], "exit_ts": t["exit_ts"], "entry": rnd(t["entry_price"]),
            "exit": rnd(t["exit_price"]), "stop": rnd(stop), "target": rnd(target), "r": round(t["r_multiple"] or 0, 3),
            "reason": t["reason"]}


def binance(sym, market, interval):
    base = sym.replace("USDT", "")
    url = f"https://www.binance.com/en/futures/{sym}" if market == "futures" else f"https://www.binance.com/en/trade/{base}_USDT?type=spot"
    tv = {"15m": "15", "4h": "240", "1d": "D", "1w": "W"}[interval]
    return {"pair": f"{base}/USDT", "market": market, "url": url,
            "tradingview": f"https://www.tradingview.com/chart/?symbol=BINANCE:{sym}{'.P' if market == 'futures' else ''}&interval={tv}"}


def candle_parts(c):
    o, h, l, cl = c[1:5]
    body, rng = abs(cl - o), h - l
    return body, rng, h - max(o, cl), min(o, cl) - l


# ---------- Study 1: pin bar at a swing level, 15m ----------

def study1():
    strategy = json.load(open(os.path.join(ROOT, "research", "pin-level-survivor.json")))
    found = []
    for sym in ["BTCUSDT", "ETHUSDT", "SOLUSDT", "XRPUSDT"]:
        rep = backtest(strategy, sym, "15m", "2026-06-29", "2026-09-27")
        rows, ind = aligned(rep, sym, "15m")
        pos = {r[0]: i for i, r in enumerate(rows)}
        for t in rep["trades"]:
            found.append({"sym": sym, "trade": t, "rows": rows, "ind": ind, "i": pos[t["entry_ts"]]})
    cases = []
    for k, x in enumerate(spread(found, PER_RULE)):
        t, rows, ind, i = x["trade"], x["rows"], x["ind"], x["i"]
        s = i - 1
        c = rows[s]
        o, h, l, cl = c[1:5]
        body, rng, up, low = candle_parts(c)
        small = max(body, 0.05 * rng)
        atr, sup, res, ema = ind["atr.value"][s], ind["sw.support"][s], ind["sw.resistance"][s], ind["trend.value"][s]
        long = t["side"] == "long"
        level = sup if long else res
        # The swing candle that made the level: the latest one with that low (high), at least 2 candles back.
        sw = next((j for j in range(s - 2, max(0, s - 400), -1) if abs((rows[j][3] if long else rows[j][2]) - level) < 1e-9), None)
        if long:
            checks = [
                check(low >= 2 * small and up <= 0.25 * rng,
                      f"Hammer: lower wick {px(low)} is {low / small:.1f}× the body {px(body)} (needs 2× or more); upper wick is {100 * up / rng:.0f}% of the candle's range (needs 25% or less)."),
                check(l <= sup + 0.3 * atr,
                      f"At the level: the low {px(l)} is within 0.3 ATR of the swing-low support {px(sup)} (the zone runs up to {px(sup + 0.3 * atr)}; ATR is {px(atr)})."),
                check(cl > sup, f"Held the level: the close {px(cl)} is back above the support {px(sup)}."),
                check(cl > ema, f"With the trend: the close {px(cl)} is above the 8-hour EMA {px(ema)}."),
            ]
            stop = t["entry_price"] * (1 - 0.012)
        else:
            checks = [
                check(up >= 2 * small and low <= 0.25 * rng,
                      f"Shooting star: upper wick {px(up)} is {up / small:.1f}× the body {px(body)} (needs 2× or more); lower wick is {100 * low / rng:.0f}% of the range (needs 25% or less)."),
                check(h >= res - 0.3 * atr,
                      f"At the level: the high {px(h)} is within 0.3 ATR of the swing-high resistance {px(res)} (the zone starts at {px(res - 0.3 * atr)}; ATR is {px(atr)})."),
                check(cl < res, f"Held the level: the close {px(cl)} is back below the resistance {px(res)}."),
                check(cl < ema, f"With the trend: the close {px(cl)} is below the 8-hour EMA {px(ema)}."),
            ]
            stop = t["entry_price"] * (1 + 0.012)
        assert all(ch["ok"] for ch in checks), (x["sym"], utc(c[0]), checks)
        target = t["entry_price"] + 2 * (t["entry_price"] - stop)
        j = next((q for q in range(i, len(rows)) if rows[q][0] >= t["exit_ts"]), len(rows) - 1)
        a, b = min(s - 60, sw - 8 if sw is not None else s - 60), min(len(rows), j + 12)
        a = max(a, s - 160)
        marks = [{"ts": c[0], "kind": "signal", "text": "hammer" if long else "shooting star"}]
        if sw is not None and sw >= a:
            marks.append({"ts": rows[sw][0], "kind": "swing", "text": "swing low" if long else "swing high"})
            marks.append({"ts": rows[sw + 2][0], "kind": "confirm", "text": "level confirmed"})
        cases.append({
            "n": k + 1, "symbol": x["sym"], "interval": "15m", "binance": binance(x["sym"], "futures", "15m"),
            "signal_ts": c[0], "signal": [rnd(o), rnd(h), rnd(l), rnd(cl)],
            "setup": ("Hammer at the swing low, with the trend" if long else "Shooting star at the swing high, with the trend"),
            "checks": checks,
            "how": (f"Open {x['sym'][:-4]}/USDT on the 15-minute chart and find the candle that opened at {utc(c[0])} UTC. "
                    f"Its low should be {px(l)} and its close {px(cl)}. The support line is the low of the swing candle at "
                    f"{utc(rows[sw][0]) if sw is not None else '(earlier)'} UTC: that candle's low is lower than the 5 candles before it and the 2 after it."
                    if long else
                    f"Open {x['sym'][:-4]}/USDT on the 15-minute chart and find the candle that opened at {utc(c[0])} UTC. "
                    f"Its high should be {px(h)} and its close {px(cl)}. The resistance line is the high of the swing candle at "
                    f"{utc(rows[sw][0]) if sw is not None else '(earlier)'} UTC: its high is above the 5 candles before it and the 2 after it."),
            "trade": trade_info(t, stop, target),
            "candles": window(rows, a, b),
            "lines": [{"label": "swing support", "values": series(ind, "sw.support", a, b), "color": "accent"},
                      {"label": "swing resistance", "values": series(ind, "sw.resistance", a, b), "color": "faint"},
                      {"label": "8-hour EMA", "values": series(ind, "trend.value", a, b), "color": "amber"}],
            "marks": marks,
        })
    return {"rule": "Pin bar at a swing level, with the 8-hour trend (Study 1's survivor)", "interval": "15m",
            "period": "29 June to 27 September 2026", "total": len(found), "cases": cases}


# ---------- Study 2: SuperTrend and Keltner on 4h ----------

TEN = ["BTCUSDT", "ETHUSDT", "SOLUSDT", "XRPUSDT", "BNBUSDT", "DOGEUSDT", "ADAUSDT", "AVAXUSDT", "LINKUSDT", "LTCUSDT"]


def study2_rule(file, kind):
    strategy = json.load(open(os.path.join(ROOT, "research", file)))
    found = []
    for sym in TEN:
        rep = backtest(strategy, sym, "4h", "2023-09-28", "2026-09-27")
        rows, ind = aligned(rep, sym, "4h")
        pos = {r[0]: i for i, r in enumerate(rows)}
        for t in rep["trades"]:
            found.append({"sym": sym, "trade": t, "rows": rows, "ind": ind, "i": pos[t["entry_ts"]]})
    cases = []
    for k, x in enumerate(spread(found, PER_RULE)):
        t, rows, ind, i = x["trade"], x["rows"], x["ind"], x["i"]
        s, c = i - 1, rows[i - 1]
        cl, pcl = c[4], rows[s - 1][4]
        atr = ind["atr.value"][s]
        long = t["side"] == "long"
        if kind == "supertrend":
            d0, d1, v0, v1 = ind["st.direction"][s], ind["st.direction"][s - 1], ind["st.value"][s], ind["st.value"][s - 1]
            checks = [check(d1 == (-1 if long else 1), f"Before: the candle at {utc(rows[s - 1][0])} closed at {px(pcl)}, {'below' if long else 'above'} the SuperTrend line {px(v1)}: a {'down' if long else 'up'}trend."),
                      check(d0 == (1 if long else -1), f"The flip: this candle closed at {px(cl)}, {'above' if long else 'below'} the SuperTrend line, which now sits at {px(v0)} {'under' if long else 'over'} price.")]
            stop = t["entry_price"] - 2 * atr if long else t["entry_price"] + 2 * atr
            checks.append(check(True, f"Stop 2 ATR from the entry: ATR is {px(atr)}, so the stop is {px(stop)}. Exit on the next flip the other way, or at the stop."))
            lines = [{"label": "SuperTrend", "values": series(ind, "st.value", max(0, s - 50), s + 400), "color": "accent"}]
            setup = "SuperTrend flipped up: buy" if long else "SuperTrend flipped down: sell short"
            market, note = "futures", ""
        else:
            u0, u1, mid = ind["kc.upper"][s], ind["kc.upper"][s - 1], ind["kc.middle"][s]
            checks = [check(pcl <= u1, f"Before: the candle at {utc(rows[s - 1][0])} closed at {px(pcl)}, at or below the upper Keltner band {px(u1)}."),
                      check(cl > u0, f"The breakout: this candle closed at {px(cl)}, above the upper band {px(u0)} (20 EMA {px(mid)} plus 2 × ATR 10).")]
            stop = t["entry_price"] - 3 * atr
            checks.append(check(True, f"Stop 3 ATR below the entry: ATR is {px(atr)}, so the stop is {px(stop)}. Exit when a candle closes below the middle line (the 20 EMA)."))
            lines = [{"label": "Keltner upper", "values": series(ind, "kc.upper", max(0, s - 50), s + 400), "color": "accent"},
                     {"label": "Keltner middle (20 EMA)", "values": series(ind, "kc.middle", max(0, s - 50), s + 400), "color": "amber"},
                     {"label": "Keltner lower", "values": series(ind, "kc.lower", max(0, s - 50), s + 400), "color": "faint"}]
            setup, market = "Close above the upper Keltner band: buy", "spot"
        assert all(ch["ok"] for ch in checks), (x["sym"], utc(c[0]), checks)
        j = next((q for q in range(i, len(rows)) if rows[q][0] >= t["exit_ts"]), len(rows) - 1)
        a, b = max(0, s - 50), min(len(rows), j + 10, s + 400)
        for ln in lines:
            ln["values"] = ln["values"][: b - a]
        cases.append({
            "n": k + 1, "symbol": x["sym"], "interval": "4h", "binance": binance(x["sym"], market, "4h"),
            "signal_ts": c[0], "signal": [rnd(v) for v in c[1:5]], "setup": setup, "checks": checks,
            "how": (f"Open {x['sym'][:-4]}/USDT on the 4-hour chart and find the candle that opened at {utc(c[0])} UTC (close {px(cl)}). "
                    + ("Add the SuperTrend indicator with length 10 and factor 3 (TradingView and Binance both have it): its line flips to the other side of price on this candle."
                       if kind == "supertrend" else
                       "Add a Keltner channel with length 20, multiplier 2 and ATR length 10, using an EMA: this candle is the first to close above the upper band.")),
            "trade": trade_info(t, stop),
            "candles": window(rows, a, b), "lines": lines,
            "marks": [{"ts": c[0], "kind": "signal", "text": "signal"}],
        })
    return cases, len(found)


def study2():
    st, n1 = study2_rule("supertrend-pick.json", "supertrend")
    kc, n2 = study2_rule("keltner-pick.json", "keltner")
    return {"groups": [
        {"rule": "SuperTrend (10, 3) flip on 4-hour candles, long and short (the futures pick)", "interval": "4h", "period": "September 2023 to September 2026", "total": n1, "cases": st},
        {"rule": "Keltner channel breakout on 4-hour candles, long only (the spot pick)", "interval": "4h", "period": "September 2023 to September 2026", "total": n2, "cases": kc},
    ]}


# ---------- Study 3: weekly coin rotation ----------

def study3():
    cfg = json.load(open(os.path.join(ROOT, "research", "rotation.json")))
    syms = cfg["symbols"]
    start = ms(cfg["from"])
    data = {s: {r[0]: r[4] for r in load(s, "1d") if r[0] >= start} for s in syms}
    ts = sorted({t for d in data.values() for t in d})
    close = {s: [data[s].get(t) for t in ts] for s in syms}
    hist = {}
    for s in syms:
        run, out = 0, []
        for v in close[s]:
            run = run + 1 if v is not None else 0
            out.append(run)
        hist[s] = out
    lookback, every, top, filt, min_hist = 30, 7, 5, 100, 60
    btc = close["BTCUSDT"]
    events = []
    for t in range(0, len(ts) - every, every):
        ranked = sorted(((close[s][t] / close[s][t - lookback] - 1, s) for s in syms
                         if hist[s][t] > max(min_hist, lookback) and close[s][t - lookback]), reverse=True)
        if t < filt or not ranked:
            continue
        sma = sum(btc[t - filt + 1: t + 1]) / filt
        events.append({"t": t, "ranked": ranked, "sma": sma, "on": btc[t] > sma})
    # Examples spread through rebalances; include ones where the filter kept the account in cash.
    picks = [events[round(i * (len(events) - 1) / (PER_RULE - 1))] for i in range(PER_RULE)]
    cases = []
    for k, e in enumerate(picks):
        t = e["t"]
        held = [s for _, s in e["ranked"][:top]] if e["on"] else []
        nxt = t + every
        def ret(s, a=t, b=nxt):
            x, y = close[s][a], close[s][b]
            return None if x is None or y is None else 100 * (y / x - 1)
        cases.append({
            "n": k + 1, "date": utc(ts[t], True), "next": utc(ts[nxt], True), "filter_on": e["on"],
            "btc": {"close": rnd(btc[t]), "sma100": rnd(e["sma"])},
            "ranking": [{"symbol": s, "ret30": round(100 * r, 2), "then": rnd(close[s][t - lookback]), "now": rnd(close[s][t]),
                         "held": s in held, "week": None if ret(s) is None else round(ret(s), 2)} for r, s in e["ranked"][:10]],
            "held": held, "week_avg": round(sum(ret(s) for s in held) / len(held), 2) if held else 0.0,
            "btc_week": round(ret("BTCUSDT"), 2),
            "btc_chart": [[ts[q], rnd(btc[q]), rnd(sum(btc[q - filt + 1: q + 1]) / filt)] for q in range(max(filt, t - 150), min(len(ts), t + 30))],
            "from_date": utc(ts[t - lookback], True),
        })
    return {"rule": "Every 7 days, hold the 5 coins with the best 30-day return, only while BTC closes above its 100-day average", "interval": "1d",
            "period": "January 2020 to September 2026", "total": len(events), "cases": cases}


# ---------- Study 4: weekly engulfing, daily trigger, BTC filter ----------

def weeks_of(rows):
    """Monday-to-Sunday candles, keyed by week, with the index of each week's last day."""
    out = {}
    for i, (t, o, h, l, c) in enumerate(rows):
        k = (t // DAY - 4) // 7
        if k not in out:
            out[k] = [(k * 7 + 4) * DAY, o, h, l, c, i]
        else:
            w = out[k]
            w[2], w[3], w[4], w[5] = max(w[2], h), min(w[3], l), c, i
    return out


def study4():
    strategy = json.load(open(os.path.join(ROOT, "research", "weekly-engulf-pick.json")))
    syms = json.load(open(os.path.join(ROOT, "research", "weekly-patterns.json")))["symbols"]
    found = []
    for sym in syms:
        rep = backtest(strategy, sym, "1d", "2020-05-21", "2026-09-27")
        if not rep["trades"]:
            continue
        rows, ind = aligned(rep, sym, "1d")
        pos = {r[0]: i for i, r in enumerate(rows)}
        for t in rep["trades"]:
            found.append({"sym": sym, "trade": t, "rows": rows, "ind": ind, "i": pos[t["entry_ts"]]})
    cases = []
    for k, x in enumerate(spread(found, PER_RULE)):
        t, rows, ind, i = x["trade"], x["rows"], x["ind"], x["i"]
        s, c = i - 1, rows[i - 1]
        wk = weeks_of(rows)
        key = (c[0] // DAY - 4) // 7
        # The latest completed week as of this day's close: this week if today is Sunday, else last week.
        done = key if wk[key][5] == s and (c[0] // DAY - 4) % 7 == 6 else key - 1
        w1, w0 = wk[done - 1], wk[done]
        b1, b0 = abs(w1[4] - w1[1]), abs(w0[4] - w0[1])
        atr = ind["atr.value"][s]
        btc, btc50 = ind["btc.value"][s], ind["btc50.value"][s]
        prev = rows[s - 1]
        checks = [
            check(w1[4] < w1[1], f"The week of {utc(w1[0], True)} closed lower: open {px(w1[1])}, close {px(w1[4])}."),
            check(w0[4] > w0[1] and w0[1] <= w1[4] and w0[4] >= w1[1] and b0 > b1,
                  f"The week of {utc(w0[0], True)} engulfed it: it opened at {px(w0[1])} (at or below {px(w1[4])}) and closed at {px(w0[4])} (at or above {px(w1[1])}), with a bigger body."),
            check(c[4] > c[1] and c[4] > prev[2], f"The trigger: on {utc(c[0], True)} the daily candle closed green at {px(c[4])}, above the previous day's high {px(prev[2])}."),
            check(btc > btc50, f"The BTC filter: bitcoin closed that day at {px(btc)}, above its 50-day average {px(btc50)}."),
        ]
        assert all(ch["ok"] for ch in checks), (x["sym"], utc(c[0]), checks)
        stop = t["entry_price"] - 2 * atr
        j = next((q for q in range(i, len(rows)) if rows[q][0] >= t["exit_ts"]), len(rows) - 1)
        a, b = max(0, w1[5] - 6 - 20), min(len(rows), j + 8)
        weekly = [[w[0], rnd(w[1]), rnd(w[2]), rnd(w[3]), rnd(w[4])] for kk, w in sorted(wk.items()) if done - 8 <= kk <= done + 5]
        cases.append({
            "n": k + 1, "symbol": x["sym"], "interval": "1d", "binance": binance(x["sym"], "spot", "1d"),
            "signal_ts": c[0], "signal": [rnd(v) for v in c[1:5]], "setup": "Weekly bullish engulfing, confirmed on the daily chart, with BTC in an uptrend",
            "checks": checks + [check(True, f"Stop 2 ATR below the entry: the 14-day ATR is {px(atr)}, so the stop is {px(stop)}. Sell after 28 days unless the stop is hit first.")],
            "how": (f"On Binance, open {x['sym'][:-4]}/USDT and switch to the 1W chart (weeks start Monday 00:00 UTC). Find the weeks of "
                    f"{utc(w1[0], True)} and {utc(w0[0], True)}. Then switch to 1D and find {utc(c[0], True)}: it closes above the high of {utc(prev[0], True)}. "
                    f"For the filter, open BTC/USDT on 1D with a 50-day simple moving average."),
            "trade": trade_info(t, stop),
            "candles": window(rows, a, b), "lines": [],
            "weekly": weekly, "pattern_weeks": [w1[0], w0[0]],
            "marks": [{"ts": c[0], "kind": "signal", "text": "trigger"}, {"ts": rows[w1[5] - 6][0] if w1[5] >= 6 else rows[0][0], "kind": "week", "text": "falling week"},
                      {"ts": rows[w0[5] - 6][0] if w0[5] >= 6 else rows[0][0], "kind": "week", "text": "engulfing week"}],
        })
    return {"rule": "Weekly bullish engulfing, bought on a daily close above the previous day's high, only while BTC is above its 50-day average", "interval": "1d",
            "period": "May 2020 to September 2026", "total": len(found), "cases": cases}


def main():
    out = os.path.join(ROOT, "research", "results")
    for name, fn in [("cases-1", study1), ("cases-2", study2), ("cases-3", study3), ("cases-4", study4)]:
        doc = fn()
        doc["generated"] = datetime.now(timezone.utc).strftime("%Y-%m-%d")
        with open(os.path.join(out, f"{name}.json"), "w") as f:
            json.dump(doc, f, separators=(",", ":"))
        n = sum(len(g["cases"]) for g in doc.get("groups", [doc]))
        print(f"{name}: {n} examples")


if __name__ == "__main__":
    main()
