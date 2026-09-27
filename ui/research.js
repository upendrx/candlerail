"use strict";
/*
 * Research tab: the recorded day-trading studies, how each stage filtered the
 * variants, what costs do to tight stops, and an explorer for every variant.
 * Study files are digests: pooled rows for every variant, with per-market and
 * per-trend detail fetched on demand.
 */
(() => {
const R = { list: [], studies: {}, cur: null, cost: 0, tf: "", show: "all", metric: "is", sort: { key: "ris", dir: -1 }, open: null };
const ORDER = ["intraday-reversals", "intraday-reversals-2", "confirm-other-coins", "confirm-earlier-period"];
const SHORT = { "intraday-reversals": "Round 1", "intraday-reversals-2": "Round 2", "confirm-other-coins": "Confirm: other coins", "confirm-earlier-period": "Confirm: earlier months" };

/* ---------- data helpers ---------- */
// Part arrays: [trades, wins, sum_r, r_trades, gross_win, gross_loss, return_pct]
const P = a => ({ trades: a[0], wins: a[1], sumR: a[2], rTrades: a[3], gw: a[4], gl: a[5], ret: a[6] });
const avgR = p => p.rTrades ? p.sumR / p.rTrades : null;
const pf = p => p.gl > 0 ? p.gw / p.gl : null;
const winPct = p => p.trades ? 100 * p.wins / p.trades : 0;
const both = (a, b) => ({ trades: a.trades + b.trades, wins: a.wins + b.wins, sumR: a.sumR + b.sumR, rTrades: a.rTrades + b.rTrades, gw: a.gw + b.gw, gl: a.gl + b.gl, ret: a.ret + b.ret });
function rowsOf(d) {
  return d.rows.map(r => {
    const fam = d.study.families[r[0]], is = P(r[6]), oos = P(r[7]);
    return {
      fi: r[0], family: fam.id, name: fam.name, variant: r[1], params: d.variants[r[0]][r[1]] || {}, interval: r[2], cost: r[3],
      tpd: r[4], ptpd: r[5], is, oos, all: both(is, oos), upIs: r[8], upOos: r[9], markets: r[10], feesR: r[11], selected: r[12], survived: r[13],
    };
  });
}
const rfmt = v => v == null ? "–" : rmul(v);
const rcls = v => v == null ? "" : v > 0 ? "up" : v < 0 ? "down" : "";
const med = a => { const s = a.filter(x => x != null).sort((x, y) => x - y); return s.length ? s[Math.floor((s.length - 1) / 2)] : null; };
const dday = ms => dt(ms).slice(0, 10);

/* ---------- loading ---------- */
async function load() {
  R.list = (await api("/api/studies")).sort((a, b) => ORDER.indexOf(a.id) - ORDER.indexOf(b.id));
  await Promise.all(R.list.map(async s => { R.studies[s.id] = await api("/api/studies/" + s.id); }));
  R.cur = R.list.find(s => s.id === "intraday-reversals-2")?.id || R.list[0]?.id;
  renderAll();
}
function renderAll() { verdict(); funnel(); fees(); daily(); controls(); explore(); }

/* ---------- verdict and funnel ---------- */
function stageStats(id, cost = 0) {
  const d = R.studies[id]; if (!d) return null;
  const rows = rowsOf(d).filter(r => r.cost === cost);
  const tests = d.rows.length * (d.study.symbols.length);
  return { d, rows, tests, selected: rows.filter(r => r.selected), survived: rows.filter(r => r.survived) };
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
    ${passed ? "" : " Treat any single good result below as luck until it repeats on data it has never seen."}</p></div></div>`;
}
function funnel() {
  const st = [], r1 = stageStats("intraday-reversals"), r2 = stageStats("intraday-reversals-2");
  const a = confirmation("confirm-other-coins"), b = confirmation("confirm-earlier-period");
  if (r1) st.push({ t: "Round 1 · 1m to 15m", n: r1.tests.toLocaleString() + " tests", p: `${r1.d.study.families.length} setups × ${r1.d.variants[0].length} settings, ${r1.d.study.symbols.length} coins, ${r1.d.study.intervals.join(", ")}, 2 cost levels.` });
  if (r1) st.push({ t: "Selected in round 1", n: r1.selected.length, p: "Fees on stops this tight cost 0.3R to 1.4R a trade; nothing cleared the bar.", cls: r1.selected.length ? "" : "bad" });
  if (r2) st.push({ t: "Round 2 · wider stops", n: r2.tests.toLocaleString() + " tests", p: `Stops of 0.8% to 2 ATR, ${r2.d.study.intervals.join(", ")}, holds up to 8 hours.` });
  if (r2) st.push({ t: "Selected, then survived", n: `${r2.selected.length} → ${r2.survived.length}`, p: r2.survived.length ? `Survivor: ${r2.survived.map(s => `${s.name.toLowerCase()}, ${s.interval}, stop ${s.params.stop}, ${s.params.rr}R`).join("; ")}.` : "None stayed profitable on the held-out month.", cls: r2.survived.length ? "good" : "bad" });
  if (a) st.push({ t: "Five other coins", n: rfmt(a.avg), p: `${a.trades} trades, profitable on ${a.up} of ${a.n} coins.`, cls: a.pass ? "good" : "bad" });
  if (b) st.push({ t: "The 90 days before", n: rfmt(b.avg), p: `${b.trades} trades, profitable on ${b.up} of ${b.n} coins.`, cls: b.pass ? "good" : "bad" });
  $("rs-funnel").innerHTML = `<div class="funnel">${st.map(x => `<div class="fstage ${x.cls || ""}"><span class="t">${esc(x.t)}</span><span class="n">${esc(String(x.n))}</span><p>${esc(x.p)}</p></div>`).join("")}</div>`;
}

/* ---------- costs ---------- */
function fees() {
  const d = R.studies["intraday-reversals"] || R.studies[R.cur];
  const cases = d.study.costs.map(c => ({ label: c.label, rt: 2 * (c.fee_bps + c.slippage_bps) / 100 }));
  const stops = [0.1, 0.2, 0.3, 0.5, 0.8, 1.2, 2];
  const cell = v => `<span class="fc ${v < 0.1 ? "g" : v < 0.3 ? "a" : "r"}">${fmt(v, 2)}R</span>`;
  const be = (c, t) => { const w = (1 + c) / (t + 1) * 100; return w >= 100 ? "never" : fmt(w, 0) + "%"; };
  $("rs-fees").innerHTML = `<thead><tr><th>Stop</th>${cases.map(c => `<th>${esc(c.label)}<br><span class="hint" style="margin:0">${fmt(c.rt, 2)}% round trip</span></th>`).join("")}<th>Win rate needed<br><span class="hint" style="margin:0">1:1 · 2:1, ${esc(cases[0].label)}</span></th></tr></thead><tbody>` +
    stops.map(s => `<tr><td><b>${s}%</b></td>${cases.map(c => `<td>${cell(c.rt / s)}</td>`).join("")}<td>${be(cases[0].rt / s, 1)} · ${be(cases[0].rt / s, 2)}</td></tr>`).join("") + `</tbody>`;
  const all = Object.values(R.studies).flatMap(rowsOf);
  const byTf = {};
  for (const r of all) if (r.feesR != null) (byTf[`${r.interval}|${r.cost}`] ||= []).push(r.feesR);
  const tfs = [...new Set(all.map(r => r.interval))].sort((a, b) => CAT.intervals.indexOf(a) - CAT.intervals.indexOf(b));
  $("rs-fees-measured").innerHTML = `Measured in the studies, median fees per trade: ${tfs.map(tf => `<b>${tf}</b> ${fmt(med(byTf[`${tf}|0`] || []) ?? 0, 2)}R`).join(" · ")} (${esc(cases[0].label)}; slippage comes on top). A 0.1% stop with a 0.2% target needs a ${be(cases[0].rt / 0.1, 2)} win rate just to break even.`;
}
function daily() {
  const r2 = stageStats("intraday-reversals-2"), a = confirmation("confirm-other-coins"), b = confirmation("confirm-earlier-period");
  const s = r2?.survived[0] || r2?.selected[0];
  if (!s) { $("rs-daily").innerHTML = `<p class="hint">No variant was selected, so there is nothing to size.</p>`; return; }
  const row = (label, sub, r, tpd) => { const v = r == null ? null : tpd * r; return `<div class="dailyrow"><div>${label}<br><span>${sub}</span></div><div style="text-align:right"><span>per trade</span><br><b class="${rcls(r)}">${rfmt(r)}</b></div><div style="text-align:right"><span>per day, 1% risk</span><br><b class="${rcls(v)}">${v == null ? "–" : pct(v)}</b></div></div>`; };
  $("rs-daily").innerHTML = `<p style="margin:0 0 6px;font-size:14px"><b>${esc(s.name)}</b>, ${s.interval}, stop ${esc(s.params.stop)}, target ${esc(s.params.rr)}R${s.params.trend ? ", trend " + esc(s.params.trend) : ""}. About ${fmt(s.ptpd, 1)} trades a day across ${s.markets} coins.</p>` +
    row("Selection period", "the 60 days it was chosen on", avgR(s.is), s.ptpd) +
    row("Held-out month", "the 30 days after", avgR(s.oos), s.ptpd) +
    (a ? row("Five other coins", "same 90 days, never tested", a.avg, a.ptpd) : "") +
    (b ? row("The 90 days before", "same coins, never tested", b.avg, b.ptpd) : "") +
    `<p class="hint">This is how a lucky result decays: the further from the data it was chosen on, the closer to the cost of trading it gets.</p>`;
}

/* ---------- explorer ---------- */
function controls() {
  $("rs-study").innerHTML = R.list.map(s => `<button data-v="${s.id}" class="${s.id === R.cur ? "on" : ""}">${esc(SHORT[s.id] || s.name)}</button>`).join("");
  $("rs-study").querySelectorAll("button").forEach(b => b.onclick = () => { R.cur = b.dataset.v; R.tf = ""; R.open = null; controls(); explore(); });
  const d = R.studies[R.cur];
  $("rs-cost").innerHTML = d.study.costs.map((c, i) => `<button data-v="${i}" class="${i === R.cost ? "on" : ""}">${esc(c.label)}</button>`).join("");
  $("rs-cost").querySelectorAll("button").forEach(b => b.onclick = () => { R.cost = +b.dataset.v; controls(); explore(); });
  $("rs-tf").innerHTML = [["", "All"], ...d.study.intervals.map(i => [i, i])].map(([v, l]) => `<button data-v="${v}" class="${v === R.tf ? "on" : ""}">${l}</button>`).join("");
  $("rs-tf").querySelectorAll("button").forEach(b => b.onclick = () => { R.tf = b.dataset.v; controls(); explore(); });
  $("rs-study-desc").innerHTML = `${esc(d.study.description)} <br>Data ${dday(Math.min(...d.data.map(x => x.from)))} to ${dday(Math.max(...d.data.map(x => x.to)))}${d.study.split > 0 ? `, held out from ${dday(d.split_ts)}` : ", all of it unseen"}. Recorded ${dday(d.generated_at)}.`;
}
function segBind(id, key, after) {
  $(id).querySelectorAll("button").forEach(b => b.onclick = () => { R[key] = b.dataset.v; $(id).querySelectorAll("button").forEach(x => x.classList.toggle("on", x === b)); after(); });
}
segBind("rs-show", "show", () => explore());
segBind("rs-metric", "metric", () => heat());
$("rs-active").onchange = () => table();
function current() {
  const active = $("rs-active").checked;
  return rowsOf(R.studies[R.cur]).filter(r => r.cost === R.cost && (!R.tf || r.interval === R.tf) && (!active || r.ptpd >= 1)
    && (R.show === "all" || (R.show === "selected" ? r.selected : r.survived)));
}
function explore() { heat(); table(); detail(); }
function heat() {
  const d = R.studies[R.cur], rows = rowsOf(d).filter(r => r.cost === R.cost);
  const tfs = d.study.intervals, fams = d.study.families;
  const key = R.metric === "is" ? "is" : "oos";
  $("rs-heat-which").textContent = R.metric === "is" ? "in the selection period" : d.study.split > 0 ? "in the held-out month" : "on unseen data";
  const color = v => {
    if (v == null) return "background:var(--surface2);color:var(--faint)";
    const t = Math.max(-1, Math.min(1, v / 0.5)), a = Math.abs(t) * 0.55 + 0.06;
    return `background:${t >= 0 ? `rgba(15,159,110,${a})` : `rgba(229,72,77,${a})`};color:${Math.abs(t) > 0.55 ? "#fff" : "var(--ink)"}`;
  };
  let html = `<div class="heat" style="grid-template-columns:minmax(170px,1.4fr) repeat(${tfs.length}, minmax(80px,1fr))"><div></div>${tfs.map(t => `<div class="hh">${t}</div>`).join("")}`;
  for (const [fi, f] of fams.entries()) {
    html += `<div class="hl" title="${esc(f.idea)}">${esc(f.name)}</div>`;
    for (const tf of tfs) {
      const xs = rows.filter(r => r.fi === fi && r.interval === tf), vals = xs.map(r => avgR(r[key]));
      const m = med(vals), best = Math.max(...vals.filter(v => v != null));
      const surv = xs.filter(r => r.survived).length;
      html += `<div class="hc" style="${color(m)}" title="${xs.length} variants; best ${rfmt(isFinite(best) ? best : null)}">${rfmt(m)}<small>best ${rfmt(isFinite(best) ? best : null)}${surv ? ` · ${surv} survived` : ""}</small></div>`;
    }
  }
  $("rs-heat").innerHTML = html + `</div>`;
}
const COLS = [
  ["name", "Setup", r => esc(r.name), r => r.name],
  ["tf", "TF", r => r.interval, r => CAT.intervals.indexOf(r.interval)],
  ["stop", "Stop", r => esc(r.params.stop ?? "–"), r => r.params.stop ?? ""],
  ["rr", "Target", r => r.params.rr ? esc(r.params.rr) + "R" : "–", r => +r.params.rr || 0],
  ["trend", "Trend", r => esc(r.params.trend ?? "–"), r => r.params.trend ?? ""],
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
  const rows = current(), col = COLS.find(c => c[0] === R.sort.key) || COLS[7];
  rows.sort((a, b) => { const x = col[3](a), y = col[3](b); return (x < y ? -1 : x > y ? 1 : 0) * R.sort.dir; });
  const shown = rows.slice(0, 200);
  $("rs-table").innerHTML = `<thead><tr>${COLS.map(c => `<th data-k="${c[0]}" class="${c[4] || ""} ${c[0] === R.sort.key ? "sorted" : ""}">${c[1]}${c[0] === R.sort.key ? (R.sort.dir < 0 ? " ↓" : " ↑") : ""}</th>`).join("")}</tr></thead><tbody>` +
    shown.map((r, i) => `<tr data-i="${i}" class="${R.open && R.open.family === r.family && R.open.variant === r.variant && R.open.interval === r.interval ? "cur" : ""}">${COLS.map(c => `<td class="${c[4] || ""}">${c[2](r)}</td>`).join("")}</tr>`).join("") + `</tbody>`;
  $("rs-table-note").textContent = rows.length ? `${rows.length.toLocaleString()} variants${rows.length > 200 ? ", showing the first 200" : ""}. Click a column to sort, a row for its detail. "Coins up" is in the selection period → held out.` : "No variants match these filters.";
  $("rs-table").querySelectorAll("th").forEach(th => th.onclick = () => { const k = th.dataset.k; R.sort = { key: k, dir: R.sort.key === k ? -R.sort.dir : -1 }; table(); });
  $("rs-table").querySelectorAll("tbody tr").forEach(tr => tr.onclick = () => { const r = shown[+tr.dataset.i]; R.open = { family: r.family, variant: r.variant, interval: r.interval, cost: r.cost, row: r }; table(); detail(); $("rs-detail").scrollIntoView({ block: "nearest", behavior: "smooth" }); });
}
async function detail() {
  const o = R.open, el = $("rs-detail");
  if (!o || !R.studies[R.cur].rows.length) { el.innerHTML = ""; return; }
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
  el.innerHTML = `<div class="rs-detail">
    <div class="row" style="justify-content:space-between;align-items:start"><div><h3 style="margin:0">${esc(r.name)} · ${r.interval} · ${Object.entries(r.params).map(([k, v]) => `${esc(k)} ${esc(v)}`).join(" · ")}</h3>
      <p class="hint" style="margin:4px 0 0">${esc(st.idea || "")}</p></div>
      <div class="row"><button class="btn small primary" id="rs-bt">Backtest on BTC</button><button class="btn small" id="rs-open">Open in builder</button></div></div>
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
  b.disabled = true; b.innerHTML = `<span class="spinner" style="border-color:var(--line2);border-top-color:var(--accent)"></span> Running… the first run downloads months of candles`;
  try {
    const d = await api(`/api/studies/${R.cur}/run`, {});
    if (d.ok === false) { alert(d.errors.join("\n")); return; }
    R.studies[R.cur] = d; R.open = null; renderAll();
  } finally { b.disabled = false; b.textContent = old; }
};

let loaded = false;
window.researchShown = async () => {
  if (loaded) return;
  loaded = true;
  while (!CAT) await new Promise(r => setTimeout(r, 50));
  load();
};
if (activeTab === "research") window.researchShown();
})();
