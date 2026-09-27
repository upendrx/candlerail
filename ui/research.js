"use strict";
/*
 * Research tab: numbered studies (1.0, 1.1, ... 2.0, 2.1, ...), each asking one
 * question. Study pages explore every variant of a recorded study; summary
 * pages tell what the study found; the plan page turns Study 2 into rules for
 * a small account. Study files are digests: pooled rows for every variant,
 * with per-market detail fetched on demand.
 */
(() => {
const R = { index: null, studies: {}, carry: {}, quant: {}, qsort: { key: "ish", dir: -1 }, qshow: "all", qopen: null, scale: 1, page: null, cur: null, cost: 0, tf: "", show: "all", metric: "is", sort: { key: "ris", dir: -1 }, open: null };
const PAGE_KEY = "candlerail-research-page";

/* ---------- data helpers ---------- */
// Part arrays: [trades, wins, sum_r, r_trades, gross_win, gross_loss, return_pct]
const P = a => ({ trades: a[0], wins: a[1], sumR: a[2], rTrades: a[3], gw: a[4], gl: a[5], ret: a[6] });
const avgR = p => p.rTrades ? p.sumR / p.rTrades : null;
const pf = p => p.gl > 0 ? p.gw / p.gl : null;
const winPct = p => p.trades ? 100 * p.wins / p.trades : 0;
const both = (a, b) => ({ trades: a.trades + b.trades, wins: a.wins + b.wins, sumR: a.sumR + b.sumR, rTrades: a.rTrades + b.rTrades, gw: a.gw + b.gw, gl: a.gl + b.gl, ret: a.ret + b.ret });
function rowsOf(d) {
  return d.rows.map((r, i) => {
    const fam = d.study.families[r[0]], is = P(r[6]), oos = P(r[7]);
    return {
      i, fi: r[0], family: fam.id, name: fam.name, variant: r[1], params: d.variants[r[0]][r[1]] || {}, interval: r[2], cost: r[3],
      tpd: r[4], ptpd: r[5], is, oos, all: both(is, oos), upIs: r[8], upOos: r[9], markets: r[10], feesR: r[11], selected: r[12], survived: r[13],
      monthly: d.monthly?.[i] || [],
    };
  });
}
const rfmt = v => v == null ? "–" : rmul(v);
const rcls = v => v == null ? "" : v > 0 ? "up" : v < 0 ? "down" : "";
const med = a => { const s = a.filter(x => x != null).sort((x, y) => x - y); return s.length ? s[Math.floor((s.length - 1) / 2)] : null; };
const quant = (a, q) => { const s = [...a].sort((x, y) => x - y); return s.length ? s[Math.min(s.length - 1, Math.floor(q * (s.length - 1)))] : 0; };
const mean = a => a.length ? a.reduce((t, x) => t + x, 0) / a.length : 0;
const dday = ms => dt(ms).slice(0, 10);
const monthName = m => new Date(Date.UTC(1970 + Math.floor(m / 12), m % 12, 1)).toLocaleString("en-US", { month: "short", year: "numeric", timeZone: "UTC" });
const monthOf = ms => { const d = new Date(ms); return (d.getUTCFullYear() - 1970) * 12 + d.getUTCMonth(); };
const usd = v => (v < 0 ? "−$" : "$") + fmt(Math.abs(v), Math.abs(v) >= 100 ? 0 : 2);
const sameParams = (a, b) => Object.keys(b).every(k => a[k] === b[k]) && Object.keys(a).length === Object.keys(b).length;

/* ---------- loading and navigation ---------- */
async function load() {
  R.index = await api("/api/research");
  const ids = [...new Set(R.index.groups.flatMap(g => g.pages.map(p => p.study)).filter(Boolean).concat(Object.keys(STUDY1)))];
  const carries = [...new Set(R.index.groups.flatMap(g => g.pages.flatMap(p => p.carry || [])).concat(["carry-recent", "carry-earlier"]))];
  const quants = [...new Set(R.index.groups.flatMap(g => g.pages.flatMap(p => [p.quant, p.also].filter(Boolean))))];
  await Promise.all([
    ...ids.map(async id => { R.studies[id] = await api("/api/studies/" + id); }),
    ...carries.map(async id => { R.carry[id] = await api("/api/carry/" + id); }),
    ...quants.map(async id => { R.quant[id] = await api("/api/quant/" + id); }),
  ]);
  let start = null; try { start = localStorage.getItem(PAGE_KEY); } catch (e) {}
  nav(); go(allPages().some(p => p.number === start) ? start : "2.0");
}
const STUDY1 = { "intraday-reversals": 1, "intraday-reversals-2": 1, "confirm-other-coins": 1, "confirm-earlier-period": 1 };
const allPages = () => R.index.groups.flatMap(g => g.pages.map(p => ({ ...p, group: g })));
function nav() {
  $("rs-nav").innerHTML = R.index.groups.map(g => `<div class="g"><b>Study ${esc(g.number)} · ${esc(g.title)}</b>${g.pages.map(p =>
    `<button data-p="${esc(p.number)}"><span class="no">${esc(p.number)}</span><span class="tt">${esc(p.title)}</span><span class="rr">${esc(p.result)}</span></button>`).join("")}</div>`).join("");
  $("rs-nav").querySelectorAll("button").forEach(b => b.onclick = () => { go(b.dataset.p); window.scrollTo({ top: 0, behavior: "smooth" }); });
}
function go(number) {
  const p = allPages().find(x => x.number === number); if (!p) return;
  R.page = p;
  try { localStorage.setItem(PAGE_KEY, number); } catch (e) {}
  $("rs-nav").querySelectorAll("button").forEach(b => b.classList.toggle("on", b.dataset.p === number));
  document.querySelectorAll("#t-research .rs-p").forEach(el => el.hidden = el.dataset.kind !== p.kind);
  const g = p.group;
  $("rs-head").innerHTML = `<div class="rs-head"><span class="num-big">${esc(p.number)} · STUDY ${esc(g.number)}</span><h2 style="font-size:24px;margin:0">${esc(p.title)}</h2></div>` +
    (p.number.endsWith(".0") ? `<div class="rs-q ${g.verdict}"><span class="q">${esc(g.question)}</span><span class="a">${esc(g.answer)}</span></div>` : `<p class="hint" style="margin:0">${esc(p.result)}.</p>`);
  if (p.kind === "overview1") { verdict(); funnel(); fees(); daily(); }
  if (p.kind === "study") { R.cur = p.study; R.tf = ""; R.open = null; R.cost = 0; R.show = "all"; R.sort = { key: R.studies[p.study].study.split > 0 ? "ris" : "roos", dir: -1 }; controls(); explore(); }
  if (p.kind === "plan") plan();
  if (p.kind === "carry") carry();
  if (p.kind === "quant") { R.qopen = null; R.qshow = "all"; quantPage(); }
  if (p.kind === "combo") comboPage();
  if (p.kind === "risk") riskPage();
  if (p.kind === "overview3") overview3();
}

/* ---------- Study 1 summary ---------- */
function stageStats(id, cost = 0) {
  const d = R.studies[id]; if (!d) return null;
  const rows = rowsOf(d).filter(r => r.cost === cost);
  return { d, rows, tests: d.rows.length * d.study.symbols.length, selected: rows.filter(r => r.selected), survived: rows.filter(r => r.survived) };
}
function confirmation(id) {
  const s = stageStats(id); if (!s || !s.rows.length) return null;
  const r = s.rows[0], p = r.oos;
  return { r, avg: avgR(p), trades: p.trades, up: r.upOos, n: r.markets, pass: (avgR(p) ?? -1) > 0 && r.upOos * 2 >= r.markets, ptpd: r.ptpd };
}
function verdict() {
  const a = confirmation("confirm-other-coins"), b = confirmation("confirm-earlier-period");
  const r1 = stageStats("intraday-reversals"), r2 = stageStats("intraday-reversals-2");
  const tests = [r1, r2, stageStats("confirm-other-coins"), stageStats("confirm-earlier-period")].filter(Boolean).reduce((t, s) => t + s.tests, 0);
  const passed = a && b && a.pass && b.pass;
  $("rs-verdict").innerHTML = `<div class="verdict ${passed ? "yes" : "no"}"><div class="big">${passed ? "✓" : "No"}</div><div>
    <h2>${passed ? "One setup held up on every test." : "No setup gave a repeatable daily profit."}</h2>
    <p>${tests.toLocaleString()} backtests. Counting futures costs, the cheaper of the two: ${r1 ? `Round 1 selected ${r1.selected.length} of ${r1.rows.length.toLocaleString()} variants. ` : ""}${r2 ? `Round 2 selected ${r2.selected.length} and ${r2.survived.length} survived the held-out month` : ""}${a && b ? `, then ${a.pass ? "held" : "failed"} on five other coins (${rfmt(a.avg)} a trade) and ${b.pass ? "held" : "failed"} on the three months before (${rfmt(b.avg)}).` : "."}
    ${passed ? "" : " Treat any single good result as luck until it repeats on data it has never seen."}</p></div></div>`;
}
function funnel() {
  const st = [], r1 = stageStats("intraday-reversals"), r2 = stageStats("intraday-reversals-2");
  const a = confirmation("confirm-other-coins"), b = confirmation("confirm-earlier-period");
  if (r1) st.push({ t: "1.1 Round 1 · 1m to 15m", n: r1.tests.toLocaleString() + " tests", p: `${r1.d.study.families.length} setups, ${r1.d.study.symbols.length} coins, ${r1.d.study.intervals.join(", ")}, 2 cost levels.` });
  if (r1) st.push({ t: "Selected in round 1", n: r1.selected.length, p: "Fees on stops this tight cost 0.3R to 1.4R a trade; nothing cleared the bar.", cls: r1.selected.length ? "" : "bad" });
  if (r2) st.push({ t: "1.2 Round 2 · wider stops", n: r2.tests.toLocaleString() + " tests", p: `Stops of 0.8% to 2 ATR, ${r2.d.study.intervals.join(", ")}, holds up to 8 hours.` });
  if (r2) st.push({ t: "Selected, then survived", n: `${r2.selected.length} → ${r2.survived.length}`, p: r2.survived.length ? `Survivor: ${r2.survived.map(s => `${s.name.toLowerCase()}, ${s.interval}, stop ${s.params.stop}, ${s.params.rr}R`).join("; ")}.` : "None stayed profitable on the held-out month.", cls: r2.survived.length ? "good" : "bad" });
  if (a) st.push({ t: "1.3 Five other coins", n: rfmt(a.avg), p: `${a.trades} trades, profitable on ${a.up} of ${a.n} coins.`, cls: a.pass ? "good" : "bad" });
  if (b) st.push({ t: "1.4 The 90 days before", n: rfmt(b.avg), p: `${b.trades} trades, profitable on ${b.up} of ${b.n} coins.`, cls: b.pass ? "good" : "bad" });
  $("rs-funnel").innerHTML = `<div class="funnel">${st.map(x => `<div class="fstage ${x.cls || ""}"><span class="t">${esc(x.t)}</span><span class="n">${esc(String(x.n))}</span><p>${esc(x.p)}</p></div>`).join("")}</div>`;
}
function fees() {
  const d = R.studies["intraday-reversals"];
  const cases = d.study.costs.map(c => ({ label: c.label, rt: 2 * (c.fee_bps + c.slippage_bps) / 100 }));
  const stops = [0.1, 0.2, 0.3, 0.5, 0.8, 1.2, 2];
  const cell = v => `<span class="fc ${v < 0.1 ? "g" : v < 0.3 ? "a" : "r"}">${fmt(v, 2)}R</span>`;
  const be = (c, t) => { const w = (1 + c) / (t + 1) * 100; return w >= 100 ? "never" : fmt(w, 0) + "%"; };
  $("rs-fees").innerHTML = `<thead><tr><th>Stop</th>${cases.map(c => `<th>${esc(c.label)}<br><span class="hint" style="margin:0">${fmt(c.rt, 2)}% round trip</span></th>`).join("")}<th>Win rate needed<br><span class="hint" style="margin:0">1:1 · 2:1, ${esc(cases[0].label)}</span></th></tr></thead><tbody>` +
    stops.map(s => `<tr><td><b>${s}%</b></td>${cases.map(c => `<td>${cell(c.rt / s)}</td>`).join("")}<td>${be(cases[0].rt / s, 1)} · ${be(cases[0].rt / s, 2)}</td></tr>`).join("") + `</tbody>`;
  const all = ["intraday-reversals", "intraday-reversals-2"].filter(id => R.studies[id]).flatMap(id => rowsOf(R.studies[id]));
  const byTf = {};
  for (const r of all) if (r.feesR != null && r.cost === 0) (byTf[r.interval] ||= []).push(r.feesR);
  const tfs = Object.keys(byTf).sort((a, b) => CAT.intervals.indexOf(a) - CAT.intervals.indexOf(b));
  $("rs-fees-measured").innerHTML = `Measured in the studies, median fees per trade: ${tfs.map(tf => `<b>${tf}</b> ${fmt(med(byTf[tf]) ?? 0, 2)}R`).join(" · ")} (${esc(cases[0].label)}; slippage comes on top). A 0.1% stop with a 0.2% target needs a ${be(cases[0].rt / 0.1, 2)} win rate just to break even.`;
}
function daily() {
  const r2 = stageStats("intraday-reversals-2"), a = confirmation("confirm-other-coins"), b = confirmation("confirm-earlier-period");
  const s = r2?.survived[0] || r2?.selected[0];
  if (!s) { $("rs-daily").innerHTML = `<p class="hint">No variant was selected, so there is nothing to size.</p>`; return; }
  const row = (label, sub, r, tpd) => { const v = r == null ? null : tpd * r; return `<div class="dailyrow"><div>${label}<br><span>${sub}</span></div><div style="text-align:right"><span>per trade</span><br><b class="${rcls(r)}">${rfmt(r)}</b></div><div style="text-align:right"><span>per day, 1% risk</span><br><b class="${rcls(v)}">${v == null ? "–" : pct(v)}</b></div></div>`; };
  $("rs-daily").innerHTML = `<p style="margin:0 0 6px;font-size:14px"><b>${esc(s.name)}</b>, ${s.interval}, stop ${esc(s.params.stop)}, target ${esc(s.params.rr)}R${s.params.trend ? ", trend " + esc(s.params.trend) : ""}. About ${fmt(s.ptpd, 1)} trades a day across ${s.markets} coins.</p>` +
    row("Selection period", "the 60 days it was chosen on", avgR(s.is), s.ptpd) + row("Held-out month", "the 30 days after", avgR(s.oos), s.ptpd) +
    (a ? row("Five other coins", "same 90 days, never tested", a.avg, a.ptpd) : "") + (b ? row("The 90 days before", "same coins, never tested", b.avg, b.ptpd) : "") +
    `<p class="hint">This is how a lucky result decays: the further from the data it was chosen on, the closer to the cost of trading it gets.</p>`;
}

/* ---------- monthly chart ---------- */
/** Bars for each month's result, a dashed line for buying and holding, and an optional split. */
function monthBars(series, opts = {}) {
  const n = series.length; if (!n) return "";
  const W = 760, H = 190, pad = 26, bh = opts.benchmark || [];
  // Holding swings far more than the strategies; keep the scale on the strategy and clip the line.
  const vals = opts.clipBenchmark ? series : [...series, ...bh];
  const hi = Math.max(1, ...vals) * 1.08, lo = Math.min(-1, ...vals) * 1.08;
  const Y = v => 10 + (hi - Math.max(lo, Math.min(hi, v))) / (hi - lo) * (H - pad - 10);
  const bw = (W - 44) / n, X = k => 40 + k * bw;
  let s = `<svg viewBox="0 0 ${W} ${H}" role="img" aria-label="${esc(opts.label || "Monthly results")}">`;
  for (const v of [hi, 0, lo]) s += `<text x="34" y="${Y(v) + 3}" font-size="10" text-anchor="end" fill="currentColor" opacity=".55">${v > 0 ? "+" : ""}${fmt(v, Math.abs(v) < 10 ? 1 : 0)}%</text>`;
  s += `<line x1="40" x2="${W - 4}" y1="${Y(0)}" y2="${Y(0)}" stroke="currentColor" stroke-opacity=".25"/>`;
  if (opts.split != null && opts.split > 0 && opts.split < n) s += `<line x1="${X(opts.split)}" x2="${X(opts.split)}" y1="4" y2="${H - pad + 4}" stroke="var(--accent)" stroke-dasharray="4 3"/><text x="${X(opts.split) + 4}" y="12" font-size="10" fill="var(--accent)">${esc(opts.splitLabel || "held out →")}</text>`;
  series.forEach((v, k) => { const y0 = Y(0), y1 = Y(v); s += `<rect x="${(X(k) + bw * 0.15).toFixed(1)}" y="${Math.min(y0, y1).toFixed(1)}" width="${Math.max(1, bw * 0.7).toFixed(1)}" height="${Math.max(1, Math.abs(y1 - y0)).toFixed(1)}" rx="1.5" fill="${v >= 0 ? "var(--up)" : "var(--down)"}" opacity=".85"><title>${esc(monthName((opts.month0 || 0) + k))}: ${pct(v)}</title></rect>`; });
  if (bh.length) s += `<polyline fill="none" stroke="var(--faint)" stroke-width="1.5" stroke-dasharray="3 2" points="${bh.slice(0, n).map((v, k) => `${(X(k) + bw / 2).toFixed(1)},${Y(v).toFixed(1)}`).join(" ")}"/>`;
  for (let k = 0; k < n; k++) { const m = (opts.month0 || 0) + k; if (m % 12 === 0) s += `<text x="${X(k) + 2}" y="${H - 8}" font-size="10" fill="currentColor" opacity=".6">${1970 + Math.floor(m / 12)}</text>`; }
  return `<div class="mbars">${s}</svg><div class="lg"><span><i style="background:var(--up)"></i>month up</span><span><i style="background:var(--down)"></i>month down</span>${bh.length ? `<span><i style="background:none;border-top:2px dashed var(--faint);height:0"></i>buy and hold, same coins${opts.clipBenchmark ? " (cut off at the chart's edge)" : ""}</span>` : ""}</div></div>`;
}
function monthStats(m) {
  const up = m.filter(x => x > 0).length;
  // Largest fall from a peak of the running total, and the longest stretch below a peak.
  let eq = 0, peak = 0, fall = 0, under = 0, longest = 0;
  for (const v of m) { eq += v; peak = Math.max(peak, eq); fall = Math.min(fall, eq - peak); under = eq < peak ? under + 1 : 0; longest = Math.max(longest, under); }
  return { n: m.length, up, mean: mean(m), worst: m.length ? Math.min(...m) : 0, best: m.length ? Math.max(...m) : 0, p25: quant(m, 0.25), p75: quant(m, 0.75), sum: m.reduce((t, x) => t + x, 0), fall, longest };
}

/* ---------- 2.0 plan ---------- */
function findRow(studyId, pick, confirm) {
  const d = R.studies[studyId]; if (!d) return null;
  return rowsOf(d).find(r => (confirm ? r.family.startsWith(pick.family + "-") : r.family === pick.family) && r.interval === pick.interval && r.cost === pick.cost && sameParams(r.params, pick.params));
}
function plan() {
  const picks = R.index.picks || [];
  const html = picks.map((pk, k) => {
    const main = findRow(pk.study, pk), conf = findRow(pk.confirm, pk, true), d = R.studies[pk.study], dc = R.studies[pk.confirm];
    if (!main || !conf) return "";
    const splitM = monthOf(d.split_ts) - d.month0;
    // The confirmation window ends where the study begins; skip the shared month.
    const early = conf.monthly.slice(0, -1), series = [...early, ...main.monthly];
    const bh = [...(dc.buy_hold_monthly || []).slice(0, -1), ...(d.buy_hold_monthly || [])];
    const all = monthStats(series), last = monthStats(main.monthly.slice(splitM + 1)), first = monthStats(early);
    const scen = [500, 1000].map(c => `<tr><td><b>${usd(c)}</b></td><td class="n ${rcls(all.mean)}">${usd(c * all.mean / 100)}</td><td class="n">${usd(c * all.p25 / 100)} to ${usd(c * all.p75 / 100)}</td><td class="n down">${usd(c * all.worst / 100)}</td><td class="n up">${usd(c * all.best / 100)}</td></tr>`).join("");
    const periods = [
      ["2020 to 2023 (never seen)", avgR(conf.oos), first, conf.upOos, conf.markets],
      ["2023 to 2025 (chosen on)", avgR(main.is), monthStats(main.monthly.slice(0, splitM)), main.upIs, main.markets],
      ["2025 to 2026 (held out)", avgR(main.oos), last, main.upOos, main.markets],
    ].map(([l, r, m, up, n]) => `<tr><td>${l}</td><td class="n ${rcls(r)}">${rfmt(r)}</td><td class="n ${rcls(m.sum)}">${pct(m.sum)}</td><td class="n">${m.up}/${m.n}</td><td class="n">${up}/${n}</td></tr>`).join("");
    return `<div class="panel pickcard">
      <div class="row" style="justify-content:space-between;align-items:start"><div><div class="eyebrow" style="margin:0 0 4px">Pick ${k + 1}</div><h2 style="margin:0">${esc(pk.title)}</h2><p class="hint" style="margin:4px 0 0">${esc(pk.why)}</p></div>
        <div class="row"><button class="btn small primary" data-pbt="${k}">Backtest on BTC</button><button class="btn small" data-pop="${k}">Open in builder</button></div></div>
      <div class="rulebox"><b>The rule.</b> ${esc(pk.rule)} Trade the 10 coins (${esc(d.study.symbols.map(s => s.replace("USDT", "")).join(", "))}) side by side, each with a tenth of the account, risking 2% of that tenth per trade.</div>
      <div class="pick-stats">
        <div><b class="${rcls(all.mean)}">${pct(all.mean)}</b><span>average month, over ${all.n} months</span></div>
        <div><b>${all.up}/${all.n}</b><span>months that made money</span></div>
        <div><b class="down">${pct(all.worst)}</b><span>worst month</span></div>
        <div><b class="down">${pct(all.fall)}</b><span>largest fall from a peak</span></div>
        <div><b>${all.longest}</b><span>longest months below a peak</span></div>
        <div><b>${fmt(main.ptpd * 30, 0)}</b><span>trades a month across the 10 coins</span></div>
        <div><b>${fmt(winPct(main.all), 0)}%</b><span>of trades win</span></div>
      </div>
      <div class="tw"><table class="ptable"><thead><tr><th>Period</th><th class="n">Average per trade</th><th class="n">Sum of months</th><th class="n">Months up</th><th class="n">Coins up</th></tr></thead><tbody>${periods}</tbody></table></div>
      ${monthBars(series, { benchmark: bh, month0: dc.month0, split: early.length + splitM, splitLabel: "last year held out →", clipBenchmark: true, label: "Monthly results of " + pk.title })}
      <div class="tw"><table class="ptable"><thead><tr><th>Account</th><th class="n">Average month</th><th class="n">Middle half of months</th><th class="n">Worst month</th><th class="n">Best month</th></tr></thead><tbody>${scen}</tbody></table></div>
      <p class="hint">Dollar figures apply the tested monthly results to the account size. They scale roughly with risk: risking twice as much per trade roughly doubles both the average and the worst month.</p>
    </div>`;
  }).join("");
  const cr = R.carry["carry-recent"], carryY = cr ? mean(cr.portfolio_always) * 12 : 3;
  $("rs-plan").innerHTML = `${R.index.pick_rule ? `<p class="hint" style="margin:0"><b>How the picks were chosen.</b> ${esc(R.index.pick_rule)} The two picks are the best by that rule for each kind of account.</p>` : ""}${html}
    <div class="grid two">
      <div class="panel prose">
        <h2>Running it with $500</h2>
        <p><b>Check once every four hours, at the candle close</b> (00:00, 04:00, 08:00 and so on, UTC). Signals only exist at the close, so nothing in between matters. The Chart Lab or any chart shows the averages.</p>
        <p><b>Size every trade from the stop.</b> With $500 over 10 coins each coin has $50, and 2% of that is a $1 risk. If the stop is 4% away, the position is $25. Binance spot's minimum order is $5, so this works. Many futures contracts need $20 to $100 per order, so a small futures account may have to trade fewer coins.</p>
        <p><b>Keep it mechanical.</b> The results above come from taking every signal. Skipping the ones that feel wrong is how most traders turn a tested rule into an untested one.</p>
        <p><b>Idle money:</b> the funding-rate carry (2.3) earned about ${fmt(carryY, 1)}% a year recently with no price exposure. That's steady, but about $1 a month on $500.</p>
      </div>
      <div class="panel prose">
        <h2>What can go wrong</h2>
        <p><b>Long losing stretches.</b> Trend following loses a little in most sideways months and makes it back in a few trending ones. Even the better pick lost money in almost half of all months and once spent over a year below its previous peak.</p>
        <p><b>Buying and holding beat it in bull markets.</b> In 2020 to 2023 holding these coins made several times more. The rules earned their place by making money when the market fell.</p>
        <p><b>The coins were picked today.</b> All ten still exist and trade; coins that collapsed, such as LUNA and FTT, aren't in the test. That flatters every strategy, buy and hold included.</p>
        <p><b>The past isn't the future,</b> and three periods are still only three periods. Start small, keep a record, and compare your live results with these every month.</p>
        <p class="hint">Research on past data, not financial advice.</p>
      </div>
    </div>`;
  $("rs-plan").querySelectorAll("[data-pop],[data-pbt]").forEach(b => b.onclick = async e => {
    const btn = e.currentTarget, pk = picks[+(btn.dataset.pop ?? btn.dataset.pbt)], row = findRow(pk.study, pk), d = R.studies[pk.study];
    const st = await api(`/api/studies/${pk.study}/strategy`, { family: row.family, variant: row.variant, interval: row.interval, cost: row.cost });
    if (!st.ok) return;
    loadStrategy(structuredClone(st.strategy));
    if (btn.dataset.pop != null) { show("build"); return; }
    const range = d.data.find(x => x.interval === row.interval && x.symbol === "BTCUSDT");
    runBacktest(btn, { symbol: "BTCUSDT", interval: row.interval, from: String(range.from), to: String(range.to + 1), capital: d.study.capital });
  });
}

/* ---------- 2.3 carry ---------- */
function carry() {
  const ids = R.page.carry || [];
  const yr = m => mean(m) * 12;
  $("rs-carry").innerHTML = `<div class="panel prose" style="max-width:none">
      <h2>How it works</h2>
      <p>Buy a coin on spot and short the same amount of its perpetual future. Price moves cancel out. Every eight hours, perpetual futures pay a <i>funding rate</i> between longs and shorts to keep their price close to spot. When more traders are long, which is most of the time in crypto, the shorts receive it. Here half the account holds the coin and half is margin for the short at 1x, and both legs pay fees and slippage to open and close. The filtered version holds only while the last seven days' funding was positive.</p>
    </div>` + ids.map(id => {
      const c = R.carry[id]; if (!c) return "";
      const coins = c.coins.map(x => `<tr><td><b>${esc(x.symbol.replace("USDT", ""))}</b></td><td class="n ${rcls(yr(x.always.monthly))}">${pct(yr(x.always.monthly))}</td><td class="n ${rcls(yr(x.filtered.monthly))}">${pct(yr(x.filtered.monthly))}</td><td class="n">${fmt(100 * x.always.negative_share, 0)}%</td><td class="n">${fmt(100 * x.filtered.held_share, 0)}%</td></tr>`).join("");
      const ms = monthStats(c.portfolio_always);
      return `<div class="panel">
        <h2>${esc(c.name)}</h2><p class="sub">${esc(c.description)} ${dday(c.from)} to ${dday(c.to)}.</p>
        <div class="pick-stats" style="margin-bottom:12px">
          <div><b class="${rcls(ms.mean)}">${pct(yr(c.portfolio_always))}</b><span>a year, held all the time</span></div>
          <div><b class="${rcls(mean(c.portfolio_filtered))}">${pct(yr(c.portfolio_filtered))}</b><span>a year, filtered</span></div>
          <div><b>${ms.up}/${ms.n}</b><span>months that made money</span></div>
          <div><b>${usd(500 * ms.mean / 100)}</b><span>average month on $500</span></div>
        </div>
        ${monthBars(c.portfolio_always, { month0: c.month0, label: c.name })}
        <div class="tw" style="margin-top:12px"><table class="ptable"><thead><tr><th>Coin</th><th class="n">Always, a year</th><th class="n">Filtered, a year</th><th class="n">Funding negative</th><th class="n">Filter held</th></tr></thead><tbody>${coins}</tbody></table></div>
      </div>`;
    }).join("") + `<div class="panel prose" style="max-width:none"><h2>Before you try it</h2>
      <p><b>It's slow money.</b> A few percent a year, about a dollar a month on $500. Its value is being steady and independent of price, not large.</p>
      <p><b>The short can be liquidated</b> if the coin rallies hard while the margin sits at 1x, so move profit from the spot side to the futures margin as price rises. Funding can also turn negative for weeks, and both legs sit on one exchange, so exchange risk is total.</p>
      <p class="hint">Binance and other exchanges offer this as a built-in "funding rate arbitrage" product; check its fees against these figures.</p></div>`;
}

/* ---------- study explorer ---------- */
function controls() {
  const d = R.studies[R.cur];
  $("rs-ex-title").textContent = `Every variant · ${d.study.name}`;
  $("rs-cost").innerHTML = d.study.costs.map((c, i) => `<button data-v="${i}" class="${i === R.cost ? "on" : ""}">${esc(c.label)}</button>`).join("");
  $("rs-cost").querySelectorAll("button").forEach(b => b.onclick = () => { R.cost = +b.dataset.v; controls(); explore(); });
  $("rs-tf").innerHTML = [["", "All"], ...d.study.intervals.map(i => [i, i])].map(([v, l]) => `<button data-v="${v}" class="${v === R.tf ? "on" : ""}">${l}</button>`).join("");
  $("rs-tf").querySelectorAll("button").forEach(b => b.onclick = () => { R.tf = b.dataset.v; controls(); explore(); });
  $("rs-show").querySelectorAll("button").forEach(x => x.classList.toggle("on", x.dataset.v === R.show));
  $("rs-study-desc").innerHTML = `${esc(d.study.description)} <br>Data ${dday(Math.min(...d.data.map(x => x.from)))} to ${dday(Math.max(...d.data.map(x => x.to)))}${d.study.split > 0 ? `, held out from ${dday(d.split_ts)}` : ", all of it unseen"}. Recorded ${dday(d.generated_at)}.`;
}
function segBind(id, key, after) {
  $(id).querySelectorAll("button").forEach(b => b.onclick = () => { R[key] = b.dataset.v; $(id).querySelectorAll("button").forEach(x => x.classList.toggle("on", x === b)); after(); });
}
segBind("rs-show", "show", () => explore());
segBind("rs-metric", "metric", () => heat());
$("rs-active").onchange = () => table();
function current() {
  const d = R.studies[R.cur], sel = d.study.selection, active = $("rs-active").checked;
  return rowsOf(d).filter(r => r.cost === R.cost && (!R.tf || r.interval === R.tf)
    && (!active || (sel.portfolio ? r.ptpd : r.tpd) >= sel.min_trades_per_day)
    && (R.show === "all" || (R.show === "selected" ? r.selected : r.survived)));
}
function explore() { heat(); table(); detail(); }
function heat() {
  const d = R.studies[R.cur], rows = rowsOf(d).filter(r => r.cost === R.cost);
  const tfs = d.study.intervals, fams = d.study.families;
  const key = R.metric === "is" && d.study.split > 0 ? "is" : "oos";
  $("rs-heat-which").textContent = key === "is" ? "in the selection period" : d.study.split > 0 ? "in the held-out period" : "on unseen data";
  const color = v => {
    if (v == null) return "background:var(--surface2);color:var(--faint)";
    const t = Math.max(-1, Math.min(1, v / 0.5)), a = Math.abs(t) * 0.55 + 0.06;
    return `background:${t >= 0 ? `rgba(15,159,110,${a})` : `rgba(229,72,77,${a})`};color:${Math.abs(t) > 0.55 ? "#fff" : "var(--ink)"}`;
  };
  let html = `<div class="heat" style="grid-template-columns:minmax(170px,1.4fr) repeat(${tfs.length}, minmax(80px,1fr))"><div></div>${tfs.map(t => `<div class="hh">${t}</div>`).join("")}`;
  for (const [fi, f] of fams.entries()) {
    const xsAll = rows.filter(r => r.fi === fi);
    if (!xsAll.length) continue;
    html += `<div class="hl" title="${esc(f.idea)}">${esc(f.name)}${fams.filter(x => x.name === f.name).length > 1 ? ` <span class="hint" style="margin:0">${esc(Object.values(xsAll[0].params).join(", "))}</span>` : ""}</div>`;
    for (const tf of tfs) {
      const xs = xsAll.filter(r => r.interval === tf), vals = xs.map(r => avgR(r[key])).filter(v => v != null);
      if (!xs.length) { html += `<div class="hc" style="${color(null)}">–</div>`; continue; }
      const m = med(vals), best = vals.length ? Math.max(...vals) : null, surv = xs.filter(r => r.survived).length;
      html += `<div class="hc" style="${color(m)}" title="${xs.length} variants">${rfmt(m)}<small>${xs.length > 1 ? `best ${rfmt(best)}` : `${xs[0].oos.trades} trades`}${surv ? ` · ${surv} survived` : ""}</small></div>`;
    }
  }
  $("rs-heat").innerHTML = html + `</div>`;
}
const COLS = [
  ["name", "Setup", r => esc(r.name), r => r.name],
  ["tf", "TF", r => r.interval, r => CAT.intervals.indexOf(r.interval)],
  ["set", "Settings", r => esc(Object.entries(r.params).map(([k, v]) => `${k} ${v}`).join(" · ")), r => JSON.stringify(r.params)],
  ["tpd", "Trades/day", r => `${fmt(r.tpd, 1)} <span class="hint" style="margin:0">(${fmt(r.ptpd, 1)} all)</span>`, r => r.tpd, "n"],
  ["win", "Win", r => fmt(winPct(r.all), 0) + "%", r => winPct(r.all), "n"],
  ["ris", "R selection", r => `<span class="${rcls(avgR(r.is))}">${rfmt(avgR(r.is))}</span>`, r => avgR(r.is) ?? -99, "n"],
  ["roos", "R held out", r => `<span class="${rcls(avgR(r.oos))}">${rfmt(avgR(r.oos))}</span>`, r => avgR(r.oos) ?? -99, "n"],
  ["pf", "PF held out", r => pf(r.oos) == null ? "–" : fmt(pf(r.oos)), r => pf(r.oos) ?? -1, "n"],
  ["up", "Coins up", r => `${r.upIs}/${r.markets} → ${r.upOos}/${r.markets}`, r => r.upOos * 10 + r.upIs, "n"],
  ["fees", "Fees", r => r.feesR == null ? "–" : fmt(r.feesR, 2) + "R", r => r.feesR ?? 99, "n"],
  ["st", "", r => r.survived ? `<span class="badge2 surv">survived</span>` : r.selected ? `<span class="badge2 sel">selected</span>` : "", r => (r.survived ? 2 : 0) + (r.selected ? 1 : 0)],
];
function table() {
  const rows = current(), col = COLS.find(c => c[0] === R.sort.key) || COLS[5];
  rows.sort((a, b) => { const x = col[3](a), y = col[3](b); return (x < y ? -1 : x > y ? 1 : 0) * R.sort.dir; });
  const shown = rows.slice(0, 200);
  $("rs-table").innerHTML = `<thead><tr>${COLS.map(c => `<th data-k="${c[0]}" class="${c[4] || ""} ${c[0] === R.sort.key ? "sorted" : ""}">${c[1]}${c[0] === R.sort.key ? (R.sort.dir < 0 ? " ↓" : " ↑") : ""}</th>`).join("")}</tr></thead><tbody>` +
    shown.map((r, i) => `<tr data-i="${i}" class="${R.open && R.open.row.i === r.i ? "cur" : ""}">${COLS.map(c => `<td class="${c[4] || ""}">${c[2](r)}</td>`).join("")}</tr>`).join("") + `</tbody>`;
  $("rs-table-note").textContent = rows.length ? `${rows.length.toLocaleString()} variants${rows.length > 200 ? ", showing the first 200" : ""}. Click a column to sort, a row for its detail. "Coins up" is in the selection period → held out.` : "No variants match these filters.";
  $("rs-table").querySelectorAll("th").forEach(th => th.onclick = () => { const k = th.dataset.k; R.sort = { key: k, dir: R.sort.key === k ? -R.sort.dir : -1 }; table(); });
  $("rs-table").querySelectorAll("tbody tr").forEach(tr => tr.onclick = () => { const r = shown[+tr.dataset.i]; R.open = { family: r.family, variant: r.variant, interval: r.interval, cost: r.cost, row: r }; table(); detail(); $("rs-detail").scrollIntoView({ block: "nearest", behavior: "smooth" }); });
}
async function detail() {
  const o = R.open, el = $("rs-detail");
  if (!o) { el.innerHTML = ""; return; }
  el.innerHTML = `<div class="rs-detail"><span class="hint" style="margin:0"><span class="spinner" style="border-color:var(--line2);border-top-color:var(--accent)"></span> Loading detail…</span></div>`;
  const req = { family: o.family, variant: o.variant, interval: o.interval, cost: o.cost };
  const [d, st] = await Promise.all([api(`/api/studies/${R.cur}/detail`, req), api(`/api/studies/${R.cur}/strategy`, req)]);
  if (R.open !== o) return;
  if (!d.ok) { el.innerHTML = `<div class="errors">${d.errors.map(esc).join("<br>")}</div>`; return; }
  const s = d.summary, r = o.row, study = R.studies[R.cur], held = study.study.split > 0;
  const mkt = s.markets.map(m => { const is = P(m.in_sample), oos = P(m.out_of_sample);
    return `<tr><td><b>${esc(m.symbol)}</b></td><td class="n">${is.trades + oos.trades}</td><td class="n ${rcls(avgR(is))}">${rfmt(avgR(is))}</td><td class="n ${rcls(avgR(oos))}">${rfmt(avgR(oos))}</td><td class="n ${rcls(is.ret + oos.ret)}">${pct(is.ret + oos.ret)}</td><td class="n">−${fmt(m.max_drawdown_pct, 1)}%</td><td class="n">${m.fees_r == null ? "–" : fmt(m.fees_r, 2) + "R"}</td></tr>`; }).join("");
  const tb = ["up", "sideways", "down"].map(k => ({ k, p: P(s.by_trend[k]) }));
  const maxAbs = Math.max(0.2, ...tb.map(t => Math.abs(avgR(t.p) ?? 0)));
  const bars = tb.map(({ k, p }) => { const v = avgR(p) ?? 0, w = Math.abs(v) / maxAbs * 50;
    return `<span>${{ up: "Uptrend", sideways: "Sideways", down: "Downtrend" }[k]}</span><div class="bar"><i style="${v >= 0 ? `left:50%;width:${w}%;background:var(--up)` : `right:50%;width:${w}%;background:var(--down)`}"></i></div><span><b class="${rcls(avgR(p))}">${rfmt(avgR(p))}</b> · ${p.trades} trades</span>`; }).join("");
  const monthly = r.monthly.length ? (() => {
    const ms = monthStats(r.monthly), splitM = held ? monthOf(study.split_ts) - study.month0 : null;
    return `<div><h3 style="margin:0 0 6px">Month by month</h3><p class="hint" style="margin:0 0 8px">All ${r.markets} coins together with equal capital: ${ms.up} of ${ms.n} months made money, average ${pct(ms.mean)}, worst ${pct(ms.worst)}, best ${pct(ms.best)}.</p>
      ${monthBars(r.monthly, { benchmark: study.buy_hold_monthly, month0: study.month0, split: splitM, clipBenchmark: true })}</div>`;
  })() : "";
  el.innerHTML = `<div class="rs-detail">
    <div class="row" style="justify-content:space-between;align-items:start"><div><h3 style="margin:0">${esc(r.name)} · ${r.interval} · ${Object.entries(r.params).map(([k, v]) => `${esc(k)} ${esc(v)}`).join(" · ")}</h3>
      <p class="hint" style="margin:4px 0 0">${esc(st.idea || "")}</p></div>
      <div class="row"><button class="btn small primary" id="rs-bt">Backtest on BTC</button><button class="btn small" id="rs-open">Open in builder</button></div></div>
    ${monthly}
    <div class="grid two" style="gap:16px">
      <div class="tw"><table><thead><tr><th>Coin</th><th class="n">Trades</th><th class="n">R ${held ? "selection" : "–"}</th><th class="n">R ${held ? "held out" : "unseen"}</th><th class="n">Sum</th><th class="n">Max DD</th><th class="n">Fees</th></tr></thead><tbody>${mkt}</tbody></table></div>
      <div><h3 style="margin:0 0 8px">By market trend at entry</h3><div class="trendbars">${bars}</div>
        <p class="hint">The coin's move over the 24 hours before each trade: above +1.5% is an uptrend, below −1.5% a downtrend. Whole period, all coins.${d.recorded ? "" : " Recomputed on the study's recorded data."}</p></div>
    </div></div>`;
  $("rs-open").onclick = () => { if (st.ok) { loadStrategy(structuredClone(st.strategy)); show("build"); } };
  $("rs-bt").onclick = e => {
    if (!st.ok) return;
    const range = study.data.find(x => x.interval === o.interval && x.symbol === "BTCUSDT") || study.data.find(x => x.interval === o.interval);
    loadStrategy(structuredClone(st.strategy));
    runBacktest(e.currentTarget, { symbol: range.symbol, interval: o.interval, from: String(range.from), to: String(range.to + 1), capital: study.study.capital });
  };
}
$("rs-rerun").onclick = async e => {
  const b = e.currentTarget, old = b.textContent;
  b.disabled = true; b.innerHTML = `<span class="spinner" style="border-color:var(--line2);border-top-color:var(--accent)"></span> Running… the first run downloads the candles`;
  try {
    const d = await api(`/api/studies/${R.cur}/run`, {});
    if (d.ok === false) { alert(d.errors.join("\n")); return; }
    R.studies[R.cur] = d; R.open = null; controls(); explore();
  } finally { b.disabled = false; b.textContent = old; }
};


/* ---------- Study 3: monthly series ---------- */
const toMap = (m0, arr) => Object.fromEntries(arr.map((v, i) => [m0 + i, v]));
function pickSeries(k) {
  const pk = R.index.picks[k], main = findRow(pk.study, pk), conf = findRow(pk.confirm, pk, true);
  if (!main || !conf) return null;
  return { ...toMap(R.studies[pk.confirm].month0, conf.monthly.slice(0, -1)), ...toMap(R.studies[pk.study].month0, main.monthly) };
}
function quantVariant(id, params) { return R.quant[id]?.variants.find(v => sameParams(v.params, params)); }
function componentSeries(c) {
  if (c.kind === "pick") return pickSeries(c.pick);
  if (c.kind === "quant") { const v = quantVariant(c.id, c.params); return v ? toMap(R.quant[c.id].month0, v.monthly) : null; }
  if (c.kind === "carry") { const a = R.carry["carry-earlier"], b = R.carry["carry-recent"]; return a && b ? { ...toMap(a.month0, a.portfolio_always.slice(0, -1)), ...toMap(b.month0, b.portfolio_always) } : null; }
  return null;
}
/** Compounded statistics of monthly % returns. */
function mstats(x) {
  let eq = 1, pk = 1, dd = 0, under = 0, longest = 0;
  for (const v of x) { eq *= 1 + v / 100; pk = Math.max(pk, eq); dd = Math.min(dd, eq / pk - 1); under = eq < pk ? under + 1 : 0; longest = Math.max(longest, under); }
  const m = mean(x), sd = Math.sqrt(mean(x.map(v => (v - m) ** 2)));
  return { avg: m, worst: x.length ? Math.min(...x) : 0, best: x.length ? Math.max(...x) : 0, up: x.filter(v => v > 0).length, n: x.length, dd: 100 * dd, total: 100 * (eq - 1), sd, longest, p25: quant(x, 0.25), p75: quant(x, 0.75) };
}
function corrOf(x, y) {
  const mx = mean(x), my = mean(y); let a = 0, b = 0, c = 0;
  for (let i = 0; i < x.length; i++) { a += (x[i] - mx) * (y[i] - my); b += (x[i] - mx) ** 2; c += (y[i] - my) ** 2; }
  return b > 0 && c > 0 ? a / Math.sqrt(b * c) : 0;
}
function comboData() {
  const cfg = R.index.combo; if (!cfg) return null;
  const comps = cfg.components.map(c => ({ ...c, s: componentSeries(c) })), extras = cfg.extras.map(c => ({ ...c, s: componentSeries(c) }));
  if (comps.some(c => !c.s)) return null;
  const rot = R.quant.rotation, btc = toMap(rot.month0, rot.benchmark_monthly);
  const lastFull = monthOf(Date.now()) - 1;
  const months = Object.keys(comps[0].s).map(Number).filter(m => m <= lastFull && comps.every(c => m in c.s) && extras.every(c => !c.s || m in c.s) && m in btc).sort((a, b) => a - b);
  const [y, mo] = cfg.split.split("-").map(Number), split = (y - 1970) * 12 + mo - 1;
  const col = s => months.map(m => s[m] ?? 0);
  const inIS = months.map(m => m < split);
  const vol = x => { const v = x.filter((_, i) => inIS[i]); const m = mean(v); return Math.sqrt(mean(v.map(a => (a - m) ** 2))); };
  const cols = comps.map(c => col(c.s)), inv = cols.map(x => 1 / vol(x)), wsum = inv.reduce((a, b) => a + b, 0), w = inv.map(v => v / wsum);
  const mix = months.map((_, i) => cols.reduce((t, x, j) => t + w[j] * x[i], 0));
  return { cfg, comps, extras, months, split, w, cols, mix, btc: col(btc), inIS, extraCols: extras.map(c => c.s ? col(c.s) : null) };
}
const oosOf = (x, d) => x.filter((_, i) => !d.inIS[i]);

/* ---------- 3.0 ---------- */
function overview3() {
  const d = comboData();
  const pros = [
    ["Market making", "Quoting both sides of the order book all day and earning the spread, often with exchange fee rebates. Needs very low fees, fast connections and constant risk control.", "no", "not for one account"],
    ["Speed arbitrage", "Buying on one exchange and selling on another in milliseconds when prices differ. Needs co-located servers and deep capital on many exchanges.", "no", "not for one account"],
    ["Funding and basis at scale", "The carry trade in 2.3 run with millions at VIP fee tiers, moving between coins and exchanges as rates change. It works for a small account too, just slowly.", "meh", "small but real"],
    ["Liquidation and news speed", "Trading the seconds after large liquidations or announcements. Needs fast data feeds and execution; slower traders pay the fast ones.", "no", "not for one account"],
    ["Systematic trend and momentum", "Rules like Study 2's and coin rotation, run on many markets at once and sized as a portfolio. Checks every few hours or once a week: made for a bot.", "yes", "usable"],
  ];
  const q = id => R.quant[id], cnt = id => { const x = q(id); if (!x) return ""; const s = x.variants.filter(v => v.selected).length, v = x.variants.filter(v => v.survived).length; return `${x.variants.length} variants, ${s} selected, ${v} survived`; };
  const tests = [
    ["3.2 Coin rotation", cnt("rotation"), "yes", "Holding the strongest coins, with a BTC trend filter, held up in the held-out years; drawdowns are large on its own."],
    ["3.3 Fear & Greed sentiment", cnt("sentiment"), "meh", "The survivors are mostly a plain trend filter; the index itself added little."],
    ["3.4 Time of day and weekday", cnt("seasonality"), "no", "Hour patterns reversed after the split even before costs; weekday survivors are mostly market exposure."],
    ["3.5 Pairs trading", cnt("pairs"), "no", "Crypto pairs didn't revert reliably enough to pay the costs of both legs."],
  ];
  let plan = "";
  if (d) {
    const all = mstats(d.mix), oos = mstats(oosOf(d.mix, d)), b = mstats(d.btc);
    const scen = [500, 1000, 5000].map(c => `<tr><td><b>${usd(c)}</b></td><td class="n ${rcls(all.avg)}">${usd(c * all.avg / 100)}</td><td class="n">${usd(c * all.p25 / 100)} to ${usd(c * all.p75 / 100)}</td><td class="n down">${usd(c * all.worst / 100)}</td><td class="n down">${usd(c * all.dd / 100)}</td></tr>`).join("");
    plan = `<div class="panel pickcard">
      <div><div class="eyebrow" style="margin:0 0 4px">The combined plan</div><h2 style="margin:0">Trend following plus weekly coin rotation</h2><p class="hint" style="margin:4px 0 0">${esc(d.cfg.rule)}</p></div>
      <div class="rulebox"><b>${fmt(100 * d.w[0], 0)}% of the account: trend.</b> ${esc(R.index.picks[0].rule)} Ten coins, each with a tenth of this part, risking 2% of that tenth per trade.<br><br><b>${fmt(100 * d.w[1], 0)}% of the account: rotation.</b> ${esc(d.cfg.rotation_rule)}</div>
      <div class="pick-stats">
        <div><b class="${rcls(all.avg)}">${pct(all.avg)}</b><span>average month, ${all.n} months</span></div>
        <div><b class="${rcls(oos.avg)}">${pct(oos.avg)}</b><span>average month since the split (${oos.n} months, never used to choose)</span></div>
        <div><b>${all.up}/${all.n}</b><span>months that made money</span></div>
        <div><b class="down">${pct(all.worst)}</b><span>worst month</span></div>
        <div><b class="down">${pct(all.dd)}</b><span>largest fall from a peak</span></div>
        <div><b>${all.longest}</b><span>longest months below a peak</span></div>
      </div>
      ${monthBars(d.mix, { benchmark: d.btc, month0: d.months[0], split: d.months.indexOf(d.split), splitLabel: "weights fixed; held out →", clipBenchmark: true, label: "Combined plan month by month" })}
      <div class="tw"><table class="ptable"><thead><tr><th>Account</th><th class="n">Average month</th><th class="n">Middle half of months</th><th class="n">Worst month</th><th class="n">Largest fall</th></tr></thead><tbody>${scen}</tbody></table></div>
      <p class="hint">Buying and holding BTC over the same months: average ${pct(b.avg)}, worst month ${pct(b.worst)}, largest fall ${pct(b.dd)}. See 3.1 for what changes at other risk levels, and 3.6 for how the two parts combine.</p>
    </div>`;
  }
  $("rs-ov3").innerHTML = `<div class="panel"><h2>Where the big algorithmic profits come from</h2><p class="sub">Screenshots of huge daily gains are usually one of these, or leverage that works until it doesn't.</p>
      <div class="cards3">${pros.map(([t, p, c, l]) => `<div><span class="tag2 ${c}">${esc(l)}</span><b class="t">${esc(t)}</b><p>${esc(p)}</p></div>`).join("")}</div></div>
    <div class="panel"><h2>What Study 3 tested</h2><p class="sub">Six years of daily data (hourly for time of day), 43 coins including ones that collapsed or were delisted. Chosen before July 2024, judged after. ${esc(q("rotation")?.selection || "")}</p>
      <div class="cards3">${tests.map(([t, n, c, p]) => `<div><span class="tag2 ${c}">${c === "yes" ? "held up" : c === "meh" ? "weak" : "didn't hold"}</span><b class="t">${esc(t)}</b><p><b>${esc(n)}.</b> ${esc(p)}</p></div>`).join("")}</div></div>
    ${plan}
    <div class="grid two">
      <div class="panel prose"><h2>Running it as a bot on Binance</h2>
        <p><b>Two schedules, no chart watching.</b> Every 4 hours, at the candle close, the bot checks SuperTrend on the ten coins and opens, closes or reverses futures positions. Every Monday at 00:00 UTC it ranks the coins and rebalances the rotation on spot.</p>
        <p><b>API keys:</b> create them in your Binance account with trading enabled and <b>withdrawals disabled</b>, restricted to your computer's IP address. Keys are free; no paid data or AI service is needed.</p>
        <p><b>Start in paper mode.</b> Let the bot log the orders it would place for a few weeks and compare them with these backtests before real money.</p>
        <p class="hint">candlerail doesn't place orders yet; a paper and live runner for these two rules is the next step.</p></div>
      <div class="panel prose"><h2>Honest limits</h2>
        <p><b>This isn't a daily income.</b> ${d ? (() => { const a = mstats(d.mix); return `Even the combined plan lost money in ${a.n - a.up} of ${a.n} months and once went ${a.longest} months without a new high.`; })() : ""}</p>
        <p><b>Six years is two big crypto cycles.</b> Momentum and trend are among the most documented effects in markets, which is why they're worth trusting more than a pattern found by searching, but they can go quiet for years.</p>
        <p><b>More risk isn't free.</b> 3.1 shows how returns and drawdowns scale together, and how often a year would end down.</p>
        <p class="hint">Research on past data, not financial advice.</p></div>
    </div>`;
}

/* ---------- 3.1 ---------- */
function riskPage() {
  const d = comboData(); if (!d) { $("rs-risk").innerHTML = ""; return; }
  const base = { combo: d.mix, trend: d.cols[0] };
  const rng = seed => () => (seed = (seed * 1664525 + 1013904223) % 4294967296) / 4294967296;
  const sim = (x, k) => {
    // 5,000 random years made of 12 months drawn from the record.
    const r = rng(7), out = [], falls = [];
    for (let i = 0; i < 5000; i++) {
      let eq = 1, pk = 1, dd = 0;
      for (let j = 0; j < 12; j++) { eq *= 1 + k * x[Math.floor(r() * x.length)] / 100; pk = Math.max(pk, eq); dd = Math.min(dd, eq / pk - 1); }
      out.push(100 * (eq - 1)); falls.push(100 * dd);
    }
    return { med: quant(out, 0.5), p5: quant(out, 0.05), p95: quant(out, 0.95), down: out.filter(v => v < 0).length / 50, fall30: falls.filter(v => v <= -30).length / 50, fall50: falls.filter(v => v <= -50).length / 50 };
  };
  const row = (x, k) => { const m = mstats(x.map(v => v * k)), s = sim(x, k);
    return `<tr${k === 1 ? ` style="background:var(--accent-soft)"` : ""}><td><b>${fmt(k, k % 1 ? 1 : 0)}×</b></td><td class="n ${rcls(m.avg)}">${pct(m.avg)}</td><td class="n down">${pct(m.worst)}</td><td class="n down">${pct(m.dd)}</td><td class="n">${pct(s.med)}</td><td class="n">${pct(s.p5)}</td><td class="n">${fmt(s.down, 0)}%</td><td class="n">${fmt(s.fall30, 0)}%</td><td class="n">${fmt(s.fall50, 0)}%</td><td class="n">${usd(500 * m.avg / 100)}</td></tr>`; };
  const scales = [0.5, 1, 1.5, 2, 3, 5];
  const table = (x, title, note) => `<div class="panel"><h2>${title}</h2><p class="sub">${note}</p><div class="tw"><table class="ptable"><thead><tr><th>Scale</th><th class="n">Average month</th><th class="n">Worst month</th><th class="n">Largest fall</th><th class="n">Typical year</th><th class="n">Bad year (worst 5%)</th><th class="n">Years down</th><th class="n">Falls 30%+ in a year</th><th class="n">Falls 50%+</th><th class="n">Average month on $500</th></tr></thead><tbody>${scales.map(k => row(x, k)).join("")}</tbody></table></div></div>`;
  const kelly = x => { const m = mean(x) / 100, v = mean(x.map(a => (a / 100 - m) ** 2)); return v > 0 ? m / v : 0; };
  $("rs-risk").innerHTML = `<div class="panel prose" style="max-width:none"><h2>Returns and losses scale together</h2>
      <p>The monthly results on these pages come from small positions: the trend rule risks 2% of each coin's tenth of the account, 0.2% of the whole account per trade. Doubling every position roughly doubles the average month, <b>and</b> the worst month, and the largest fall grows faster than that because losses compound. The "typical year" and "bad year" columns come from 5,000 simulated years, each made of 12 months drawn at random from the record. Real losing months tend to come in clusters, so real bad years can be worse than these.</p>
      <p>By the Kelly formula applied to the monthly record, growth would be fastest at about ${fmt(kelly(d.mix), 1)}× for the combined plan and ${fmt(kelly(d.cols[0]), 1)}× for the trend rule alone. That assumes the future looks exactly like the past, which it won't, so professionals use a quarter to a half of Kelly at most.</p></div>
    ${table(base.combo, "The combined plan (3.0)", "1× is the plan as tested: trend and rotation weighted as in 3.6.")}
    ${table(base.trend, "The trend rule alone (Study 2)", "1× risks 0.2% of the account per trade, as tested in 2.1.")}`;
}

/* ---------- 3.2 to 3.5 ---------- */
const QCOLS = [
  ["set", "Settings", v => esc(Object.entries(v.params).map(([k, x]) => `${k} ${x}`).join(" · ")), v => JSON.stringify(v.params)],
  ["isa", "Year, before", v => `<span class="${rcls(v.in_sample.annual_pct)}">${pct(v.in_sample.annual_pct)}</span>`, v => v.in_sample.annual_pct, "n"],
  ["ish", "Sharpe, before", v => fmt(v.in_sample.sharpe), v => v.in_sample.sharpe, "n"],
  ["isd", "Fall, before", v => pct(v.in_sample.max_drawdown_pct), v => v.in_sample.max_drawdown_pct, "n"],
  ["osa", "Year, after", v => `<span class="${rcls(v.out_of_sample.annual_pct)}">${pct(v.out_of_sample.annual_pct)}</span>`, v => v.out_of_sample.annual_pct, "n"],
  ["osh", "Sharpe, after", v => fmt(v.out_of_sample.sharpe), v => v.out_of_sample.sharpe, "n"],
  ["osd", "Fall, after", v => pct(v.out_of_sample.max_drawdown_pct), v => v.out_of_sample.max_drawdown_pct, "n"],
  ["tr", "Trades/month", v => fmt(v.trades_per_month, 1), v => v.trades_per_month, "n"],
  ["st", "", v => v.survived ? `<span class="badge2 surv">survived</span>` : v.selected ? `<span class="badge2 sel">selected</span>` : "", v => (v.survived ? 2 : 0) + (v.selected ? 1 : 0)],
];
function quantTable(q, el, limit, onRow) {
  const rows = q.variants.filter(v => R.qshow === "all" || (R.qshow === "selected" ? v.selected : v.survived));
  const col = QCOLS.find(c => c[0] === R.qsort.key) || QCOLS[2];
  rows.sort((a, b) => { const x = col[3](a), y = col[3](b); return (x < y ? -1 : x > y ? 1 : 0) * R.qsort.dir; });
  const shown = rows.slice(0, limit);
  el.innerHTML = `<table class="qtable"><thead><tr>${QCOLS.map(c => `<th data-k="${c[0]}" class="${c[4] || ""} ${c[0] === R.qsort.key ? "sorted" : ""}" style="cursor:pointer">${c[1]}${c[0] === R.qsort.key ? (R.qsort.dir < 0 ? " ↓" : " ↑") : ""}</th>`).join("")}</tr></thead><tbody>` +
    shown.map((v, i) => `<tr data-i="${i}" class="${R.qopen === v ? "cur" : ""}">${QCOLS.map(c => `<td class="${c[4] || ""}">${c[2](v)}</td>`).join("")}</tr>`).join("") + `</tbody></table>`;
  el.querySelectorAll("th").forEach(th => th.onclick = () => { const k = th.dataset.k; R.qsort = { key: k, dir: R.qsort.key === k ? -R.qsort.dir : -1 }; quantPage(); });
  if (onRow) el.querySelectorAll("tbody tr").forEach(tr => tr.onclick = () => onRow(shown[+tr.dataset.i]));
  return rows.length;
}
function quantPage() {
  const q = R.quant[R.page.quant], also = R.page.also ? R.quant[R.page.also] : null;
  if (!q) { $("rs-quant").innerHTML = ""; return; }
  const sel = q.variants.filter(v => v.selected).length, surv = q.variants.filter(v => v.survived).length;
  const splitIdx = monthOf(q.split_ts) - q.month0;
  const detail = R.qopen ? (() => { const v = R.qopen, m = mstats(v.monthly);
    return `<div class="rs-detail"><h3 style="margin:0">${esc(Object.entries(v.params).map(([k, x]) => `${k} ${x}`).join(" · "))}</h3>
      <p class="hint" style="margin:0">${m.up} of ${m.n} months made money, average ${pct(m.avg)}, worst ${pct(m.worst)}, largest fall ${pct(m.dd)}. ${fmt(v.trades_per_month, 1)} positions opened a month; average exposure ${fmt(100 * v.exposure, 0)}% of the account.</p>
      ${monthBars(v.monthly, { benchmark: q.benchmark_monthly, month0: q.month0, split: splitIdx, clipBenchmark: true })}</div>`; })() : "";
  $("rs-quant").innerHTML = `<div class="panel">
      <p style="margin:0 0 10px;color:var(--ink2)">${esc(q.description)}</p>
      <div class="pick-stats" style="margin-bottom:12px">
        <div><b>${q.variants.length}</b><span>variants</span></div><div><b>${sel}</b><span>selected before ${dday(q.split_ts)}</span></div><div><b class="${surv ? "up" : "down"}">${surv}</b><span>survived after it</span></div>
        <div><b>${pct(q.benchmark_in_sample.annual_pct)} / ${pct(q.benchmark_out_of_sample.annual_pct)}</b><span>BTC buy and hold a year, before / after</span></div>
      </div>
      <p class="hint" style="margin:0 0 10px">${esc(q.selection)} Data ${dday(q.from)} to ${dday(q.to)}.</p>
      ${q.notes?.length ? `<p class="hint" style="margin:0 0 10px">${q.notes.map(esc).join("<br>")}</p>` : ""}
      <div class="row" style="margin-bottom:8px"><div class="segc" id="rs-qshow"><button data-v="all">All</button><button data-v="selected">Selected</button><button data-v="survived">Survived</button></div></div>
      <div class="tw" style="max-height:520px;overflow-y:auto" id="rs-qt"></div>
      <p class="hint" id="rs-qnote"></p>
      ${detail}
    </div>
    ${also ? `<div class="panel"><h2>${esc(also.name)}</h2><p class="sub">${esc(also.description)} ${also.variants.filter(v => v.selected).length} selected, ${also.variants.filter(v => v.survived).length} survived.</p><div class="tw" style="max-height:360px;overflow-y:auto" id="rs-qt2"></div></div>` : ""}`;
  $("rs-qshow").querySelectorAll("button").forEach(b => { b.classList.toggle("on", b.dataset.v === R.qshow); b.onclick = () => { R.qshow = b.dataset.v; quantPage(); }; });
  const n = quantTable(q, $("rs-qt"), 200, v => { R.qopen = v; quantPage(); });
  $("rs-qnote").textContent = `${n} variants${n > 200 ? ", showing 200" : ""}. "Before" is the selection period, "after" is from ${dday(q.split_ts)}. Click a row for its months.`;
  if (also) quantTable(also, $("rs-qt2"), 30);
}

/* ---------- 3.6 ---------- */
function comboPage() {
  const d = comboData(); if (!d) { $("rs-combo").innerHTML = ""; return; }
  const names = [...d.comps.map(c => c.label), ...d.extras.filter(c => c.s).map(c => c.label), "Buy and hold BTC"];
  const cols = [...d.cols, ...d.extraCols.filter(Boolean), d.btc];
  const corr = `<table class="ptable corr"><thead><tr><th></th>${names.map((n, i) => `<th title="${esc(n)}">${i + 1}</th>`).join("")}</tr></thead><tbody>${names.map((n, i) => `<tr><td>${i + 1}. ${esc(n)}</td>${cols.map((c, j) => { const v = corrOf(cols[i], c), a = Math.abs(v); return `<td style="background:rgba(91,91,214,${i === j ? 0 : a * 0.35})">${i === j ? "–" : fmt(v, 2)}</td>`; }).join("")}</tr>`).join("")}</tbody></table>`;
  const line = (label, x) => { const a = mstats(x), o = mstats(oosOf(x, d)); return `<tr><td><b>${esc(label)}</b></td><td class="n ${rcls(a.avg)}">${pct(a.avg)}</td><td class="n down">${pct(a.worst)}</td><td class="n down">${pct(a.dd)}</td><td class="n">${a.up}/${a.n}</td><td class="n ${rcls(o.avg)}">${pct(o.avg)}</td><td class="n down">${pct(o.dd)}</td></tr>`; };
  $("rs-combo").innerHTML = `<div class="panel prose" style="max-width:none"><h2>Why combine</h2>
      <p>Two strategies that make money in different months smooth each other out. When they're weakly correlated, the combination's worst stretches are shallower than either's, so the whole can be run at a higher size for the same pain. That, not a secret signal, is how systematic funds get more return per unit of risk.</p>
      <p class="hint">${esc(d.cfg.rule)}</p></div>
    <div class="panel"><h2>Weights</h2><div class="pick-stats">${d.comps.map((c, i) => `<div><b>${fmt(100 * d.w[i], 0)}%</b><span>${esc(c.label)}</span></div>`).join("")}</div></div>
    <div class="panel"><h2>Results, month by month</h2><p class="sub">${d.months.length} months, ${monthName(d.months[0])} to ${monthName(d.months[d.months.length - 1])}; "after" is from ${monthName(d.split)}, never used for the weights.</p>
      <div class="tw"><table class="ptable"><thead><tr><th></th><th class="n">Average month</th><th class="n">Worst month</th><th class="n">Largest fall</th><th class="n">Months up</th><th class="n">Average month, after</th><th class="n">Largest fall, after</th></tr></thead><tbody>
      ${d.comps.map((c, i) => line(c.label, d.cols[i])).join("")}${line("Combined", d.mix)}${d.extras.map((c, i) => d.extraCols[i] ? line(c.label, d.extraCols[i]) : "").join("")}${line("Buy and hold BTC", d.btc)}</tbody></table></div>
      ${monthBars(d.mix, { benchmark: d.btc, month0: d.months[0], split: d.months.indexOf(d.split), splitLabel: "held out →", clipBenchmark: true, label: "Combined month by month" })}</div>
    <div class="panel"><h2>How the parts move together</h2><p class="sub">Correlation of monthly results: 1 moves in lockstep, 0 unrelated.</p><div class="tw">${corr}</div></div>`;
}

let loaded = false;
window.researchShown = async () => {
  if (loaded) return;
  loaded = true;
  while (!CAT) await new Promise(r => setTimeout(r, 50));
  load();
};
if (activeTab === "research") window.researchShown();
})();
