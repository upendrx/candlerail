"use strict";
/*
 * Figures for the published research. Every chart is drawn from the recorded
 * result files in data/, the same files the candlerail app ships, so the
 * numbers on these pages are the numbers the studies produced.
 *
 * A page declares figures as <div data-fig="name" data-...>; draw() fills them.
 */
(() => {
const $$ = (s, el = document) => [...el.querySelectorAll(s)];
const esc = s => String(s ?? "").replace(/[&<>"]/g, c => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;" }[c]));
const css = v => getComputedStyle(document.documentElement).getPropertyValue(v).trim();
const fmt = (v, d = 2) => (v ?? 0).toLocaleString("en-US", { minimumFractionDigits: d, maximumFractionDigits: d });
const sign = v => v > 0 ? "+" : v < 0 ? "−" : "";
const pct = (v, d = 1) => v == null ? "–" : sign(v) + fmt(Math.abs(v), d) + "%";
const rf = v => v == null ? "–" : sign(v) + fmt(Math.abs(v)) + "R";
const cls = v => v == null ? "" : v > 0 ? "up" : v < 0 ? "down" : "";
const mean = a => a.length ? a.reduce((t, x) => t + x, 0) / a.length : 0;
const quant = (a, q) => { const s = [...a].sort((x, y) => x - y); return s.length ? s[Math.min(s.length - 1, Math.floor(q * (s.length - 1)))] : 0; };
const med = a => { const s = a.filter(x => x != null && isFinite(x)); return s.length ? quant(s, 0.5) : null; };
const monthOf = ms => { const d = new Date(ms); return (d.getUTCFullYear() - 1970) * 12 + d.getUTCMonth(); };
const monthName = m => new Date(Date.UTC(1970 + Math.floor(m / 12), m % 12, 1)).toLocaleString("en-US", { month: "short", year: "numeric", timeZone: "UTC" });
const day = ms => new Date(ms).toISOString().slice(0, 10);

/* ---------- data ---------- */
const cache = {};
const load = name => cache[name] ||= fetch(`data/${name}.json`).then(r => { if (!r.ok) throw new Error(name); return r.json(); });
// Part arrays: [trades, wins, sum_r, r_trades, gross_win, gross_loss, return_pct]
const P = a => ({ trades: a[0], wins: a[1], sumR: a[2], rTrades: a[3], gw: a[4], gl: a[5], ret: a[6] });
const avgR = p => p.rTrades ? p.sumR / p.rTrades : null;
function rowsOf(d) {
  return d.rows.map((r, i) => {
    const fam = d.study.families[r[0]];
    return { i, fi: r[0], family: fam.id, name: fam.name, params: d.variants[r[0]][r[1]] || {}, interval: r[2], cost: r[3], tpd: r[4], ptpd: r[5],
      is: P(r[6]), oos: P(r[7]), upIs: r[8], upOos: r[9], markets: r[10], feesR: r[11], selected: r[12], survived: r[13], monthly: d.monthly?.[i] || [] };
  });
}
const sameParams = (a, b) => Object.keys(b).every(k => a[k] === b[k]) && Object.keys(a).length === Object.keys(b).length;
// Largest fall from a peak of compounded equity, or with sum=true of the running
// total of monthly returns (the measure the app's pick tables use).
function mstats(x, sum = false) {
  let eq = 1, pk = 1, dd = 0, under = 0, longest = 0;
  for (const v of x) { eq = sum ? eq + v / 100 : eq * (1 + v / 100); pk = Math.max(pk, eq); dd = Math.min(dd, sum ? eq - pk : eq / pk - 1); under = eq < pk ? under + 1 : 0; longest = Math.max(longest, under); }
  return { avg: mean(x), worst: x.length ? Math.min(...x) : 0, best: x.length ? Math.max(...x) : 0, up: x.filter(v => v > 0).length, n: x.length, dd: 100 * dd, total: 100 * (eq - 1), longest, median: quant(x, 0.5) };
}

/* ---------- SVG helpers ---------- */
const svg = (w, h, body, label) => `<svg viewBox="0 0 ${w} ${h}" role="img" aria-label="${esc(label || "")}">${body}</svg>`;
const text = (x, y, s, o = {}) => `<text x="${x}" y="${y}" font-size="${o.size || 11}" ${o.anchor ? `text-anchor="${o.anchor}"` : ""} ${o.weight ? `font-weight="${o.weight}"` : ""} fill="${o.fill || "currentColor"}" ${o.opacity ? `opacity="${o.opacity}"` : ""}>${esc(s)}</text>`;

/** Candles as [o, h, l, c, faded?] in arbitrary units, with optional annotations. */
function candles(bars, o = {}) {
  const W = o.w || 320, H = o.h || 170, marks = o.marks || [], lines = o.lines || [];
  const top = 12 + (marks.some(m => !m.up) ? 34 : 0), bottom = H - 8 - (marks.some(m => m.up) ? 34 : 0) - (o.labels || (o.zones || []).some(z => z.label) ? 14 : 0);
  const paths = o.paths || [];
  const all = bars.flatMap(b => [b[1], b[2]]).concat(lines.map(l => l.y), paths.flatMap(p => p.pts.map(q => q[1])));
  const hi = Math.max(...all), lo = Math.min(...all);
  const Y = v => top + (hi - v) / (hi - lo || 1) * (bottom - top);
  const right = lines.length || paths.some(p => p.label) ? W - 92 : W - 8, left = o.x0 ?? 8;
  const step = Math.min(48, (right - left) / Math.max(1, bars.length)), x0 = left + step / 2;
  let s = "";
  for (const z of o.zones || []) { const xa = x0 + z.from * step - step / 2, xb = x0 + z.to * step + step / 2; s += `<rect x="${xa}" y="${top - 6}" width="${xb - xa}" height="${bottom - top + 12}" rx="8" fill="${z.color || "var(--accent-soft)"}"/>${z.label ? text((xa + xb) / 2, H - 4, z.label, { anchor: "middle", size: 10.5, opacity: 0.7 }) : ""}`; }
  for (const l of lines) s += `<line x1="${left}" x2="${W - 4}" y1="${Y(l.y)}" y2="${Y(l.y)}" stroke="${l.color || "currentColor"}" stroke-width="1.4" stroke-dasharray="${l.dash || "4 3"}" opacity="${l.opacity || 0.85}"/>${text(W - 4, Y(l.y) - 4, l.label, { anchor: "end", fill: l.color, size: 11, weight: 700 })}`;
  for (const p of paths) {
    s += `<polyline fill="none" stroke="${p.color || "var(--accent)"}" stroke-width="2" ${p.dash ? `stroke-dasharray="${p.dash}"` : ""} points="${p.pts.map(([i, y]) => `${(x0 + i * step).toFixed(1)},${Y(y).toFixed(1)}`).join(" ")}"/>`;
    if (p.label) { const [i, y] = p.pts[p.pts.length - 1]; s += text(x0 + i * step + 8, Y(y) + 4, p.label, { size: 11, weight: 700, fill: p.color }); }
  }
  bars.forEach(([op, h, l, c, faded], i) => {
    const x = x0 + i * step, col = c >= op ? "var(--up)" : "var(--down)", yt = Y(Math.max(op, c)), bh = Math.max(1.5, Math.abs(Y(op) - Y(c)));
    s += `<g opacity="${faded ? 0.3 : 1}"><line x1="${x}" x2="${x}" y1="${Y(h)}" y2="${Y(l)}" stroke="${col}" stroke-width="2"/><rect x="${x - step * 0.32}" y="${yt}" width="${step * 0.64}" height="${bh}" rx="2" fill="${col}"/></g>`;
    if (o.labels?.[i]) s += text(x, H - 4, o.labels[i], { anchor: "middle", size: 10.5, opacity: 0.65 });
  });
  for (const m of marks) {
    const x = x0 + m.i * step, y = Y(m.y), col = m.color || "var(--accent)";
    s += m.up ? `<path d="M${x} ${y + 6} l-6 10 h12 z" fill="${col}"/>${text(x, y + 30, m.label, { anchor: "middle", size: 11, weight: 700, fill: col })}`
      : `<path d="M${x} ${y - 6} l-6 -10 h12 z" fill="${col}"/>${text(x, y - 22, m.label, { anchor: "middle", size: 11, weight: 700, fill: col })}`;
  }
  return svg(W, H, s, o.label);
}

const PATTERNS = {
  hammer: { t: "Hammer", d: "A long lower wick after a fall: sellers pushed price down and buyers pushed it back.", b: [[72, 74, 60, 62, 1], [62, 64, 52, 54, 1], [49, 53, 28, 52]] },
  shooting_star: { t: "Shooting star", d: "A long upper wick after a rise: buyers tried higher and were rejected.", b: [[28, 40, 26, 38, 1], [38, 48, 36, 46, 1], [51, 72, 47, 48]] },
  bullish_engulfing: { t: "Bullish engulfing", d: "A rising candle whose body covers the previous falling one.", b: [[70, 72, 58, 60, 1], [58, 60, 46, 48], [45, 66, 43, 63]] },
  bearish_engulfing: { t: "Bearish engulfing", d: "A falling candle whose body covers the previous rising one.", b: [[30, 42, 28, 40, 1], [42, 54, 40, 52], [55, 57, 34, 37]] },
  morning_star: { t: "Morning star", d: "A big fall, a small pause, a strong rise.", b: [[74, 76, 52, 54], [50, 52, 42, 45], [47, 70, 46, 68]] },
  inside_bar: { t: "Inside bar", d: "A candle entirely inside the one before it.", b: [[40, 76, 32, 70], [64, 68, 46, 52]] },
  three_white_soldiers: { t: "Three soldiers", d: "Three strong rising candles, each opening inside the last.", b: [[26, 38, 25, 37], [33, 52, 32, 50], [46, 66, 45, 64]] },
  marubozu: { t: "Marubozu", d: "A candle with almost no wicks: one side controlled it from open to close.", b: [[50, 54, 46, 48, 1], [36, 70, 36, 70]] },
  doji: { t: "Doji", d: "Open and close almost equal: a standoff.", b: [[40, 52, 38, 50, 1], [50, 62, 48, 60, 1], [60, 72, 48, 60.6]] },
};

/* ---------- figures ---------- */
const FIG = {};

FIG.pattern = el => {
  const p = PATTERNS[el.dataset.pattern];
  el.innerHTML = `<div class="tile"><span class="tg ${el.dataset.tone || "mixed"}">${esc(el.dataset.tag || "pattern")}</span>${candles(p.b, { w: 220, h: 110, label: p.t })}<h4>${esc(p.t)}</h4><p>${esc(el.dataset.note || p.d)}</p></div>`;
};

/** A hand-drawn setup: bars, levels, zones and markers from data-spec. */
FIG.candles = el => {
  const o = JSON.parse(el.dataset.spec);
  el.innerHTML = candles(o.bars, o);
};

/** The testing timeline: when variants were chosen, when they were judged, and what else checked them. */
FIG.method = el => {
  const periods = JSON.parse(el.dataset.periods);
  const W = 900, H = 60 + 46 * periods.length;
  const t0 = Math.min(...periods.map(p => Date.parse(p.from))), t1 = Math.max(...periods.map(p => Date.parse(p.to)));
  const X = t => 150 + (Date.parse(t) - t0) / (t1 - t0) * (W - 180);
  const colors = { sel: "var(--accent)", oos: "var(--up)", unseen: "var(--warn)" };
  let s = "";
  const short = t1 - t0 < 2 * 365 * 864e5;
  for (let d = new Date(t0), k = 0; d.getTime() <= t1 && k < 400; k++) {
    const m0 = new Date(Date.UTC(d.getUTCFullYear(), d.getUTCMonth() + (short ? 1 : 12 - d.getUTCMonth()), 1)); d = m0;
    const x = X(m0.toISOString().slice(0, 10)); if (x < 150 || x > W - 20) continue;
    const lbl = short ? m0.toLocaleString("en-US", { month: "short", timeZone: "UTC" }) + (m0.getUTCMonth() === 0 ? " " + m0.getUTCFullYear() : "") : m0.getUTCFullYear();
    s += `<line x1="${x}" x2="${x}" y1="18" y2="${H - 14}" stroke="currentColor" opacity=".12"/>${text(x, 12, lbl, { anchor: "middle", opacity: 0.6 })}`;
  }
  periods.forEach((p, i) => {
    const y = 30 + i * 46, xa = X(p.from), xb = X(p.to);
    s += text(0, y + 20, p.row, { size: 12.5, weight: 600 });
    s += `<rect x="${xa}" y="${y}" width="${Math.max(4, xb - xa)}" height="30" rx="8" fill="${colors[p.kind]}" opacity=".9"/>`;
    s += text((xa + xb) / 2, y + 19, p.label, { anchor: "middle", fill: "#fff", size: 12, weight: 700 });
  });
  el.innerHTML = svg(W, H, s, "Testing timeline");
};

/** Every variant as a dot: result in the selection period against the held-out period. */
FIG.scatter = async el => {
  const d = await load(el.dataset.study), cost = +(el.dataset.cost || 0);
  const rows = rowsOf(d).filter(r => r.cost === cost && avgR(r.is) != null && avgR(r.oos) != null && (!el.dataset.min || r.ptpd >= +el.dataset.min));
  const clip = +(el.dataset.clip || 1.5);
  const W = 640, H = 420, m = 70;
  const X = v => m + (Math.max(-clip, Math.min(clip, v)) + clip) / (2 * clip) * (W - m - 16);
  const Y = v => 12 + (clip - Math.max(-clip, Math.min(clip, v))) / (2 * clip) * (H - m - 12);
  let s = `<rect x="${X(0)}" y="12" width="${X(clip) - X(0)}" height="${Y(0) - 12}" fill="var(--up-soft)"/>`;
  s += `<line x1="${m}" x2="${W - 16}" y1="${Y(0)}" y2="${Y(0)}" stroke="currentColor" opacity=".35"/><line x1="${X(0)}" x2="${X(0)}" y1="12" y2="${H - m}" stroke="currentColor" opacity=".35"/>`;
  s += `<line x1="${X(-clip)}" y1="${Y(-clip)}" x2="${X(clip)}" y2="${Y(clip)}" stroke="currentColor" stroke-dasharray="4 4" opacity=".25"/>`;
  for (const v of [-clip, -clip / 2, 0, clip / 2, clip]) { s += text(X(v), H - m + 16, rf(v), { anchor: "middle", opacity: 0.6, size: 10.5 }); s += text(m - 6, Y(v) + 4, rf(v), { anchor: "end", opacity: 0.6, size: 10.5 }); }
  s += text((W + m) / 2, H - 6, "Average per trade in the selection period", { anchor: "middle", size: 12, weight: 600 });
  s += `<text transform="translate(12 ${(H - m) / 2}) rotate(-90)" text-anchor="middle" font-size="12" font-weight="600" fill="currentColor">Average per trade afterwards</text>`;
  const order = [...rows].sort((a, b) => (a.selected - b.selected) || (a.survived - b.survived));
  for (const r of order) {
    const c = r.survived ? "var(--up)" : r.selected ? "var(--accent)" : "var(--faint)";
    s += `<circle cx="${X(avgR(r.is)).toFixed(1)}" cy="${Y(avgR(r.oos)).toFixed(1)}" r="${r.selected ? 3.6 : 2.4}" fill="${c}" opacity="${r.selected ? 0.9 : 0.35}"><title>${esc(r.name)} · ${r.interval} · ${esc(Object.values(r.params).join(", "))}: ${rf(avgR(r.is))} → ${rf(avgR(r.oos))}</title></circle>`;
  }
  s += text(X(clip) - 6, 28, "made money in both", { anchor: "end", size: 11, weight: 700, fill: "var(--up)" });
  const sel = rows.filter(r => r.selected).length, surv = rows.filter(r => r.survived).length;
  el.innerHTML = svg(W, H, s, "Selection-period result against held-out result for every variant") +
    `<div class="legend"><span><i style="background:var(--faint)"></i>${rows.length.toLocaleString()} variants</span><span><i style="background:var(--accent)"></i>${sel} selected</span><span><i style="background:var(--up)"></i>${surv} survived</span><span>Values beyond ±${clip}R are drawn at the edge.</span></div>`;
};

/** Families × timeframes: the median variant's average per trade. */
FIG.heatmap = async el => {
  const d = await load(el.dataset.study);
  const draw = (key, cost) => {
    const rows = rowsOf(d).filter(r => r.cost === cost);
    const tfs = d.study.intervals;
    const color = v => { if (v == null) return "background:var(--surface2);color:var(--faint)"; const t = Math.max(-1, Math.min(1, v / 0.5)), a = Math.abs(t) * 0.6 + 0.06; return `background:${t >= 0 ? `rgba(15,159,110,${a})` : `rgba(229,72,77,${a})`};color:${Math.abs(t) > 0.5 ? "#fff" : "var(--ink)"}`; };
    let h = `<div style="display:grid;grid-template-columns:minmax(160px,1.6fr) repeat(${tfs.length},minmax(70px,1fr));gap:3px;min-width:${220 + 80 * tfs.length}px"><div></div>${tfs.map(t => `<div style="font:650 12px var(--body);color:var(--faint);text-align:center;padding-bottom:4px">${t}</div>`).join("")}`;
    d.study.families.forEach((f, fi) => {
      const xs = rows.filter(r => r.fi === fi); if (!xs.length) return;
      h += `<div style="font-size:13px;color:var(--ink2);align-self:center;padding-right:8px" title="${esc(f.idea)}">${esc(f.name)}</div>`;
      for (const tf of tfs) {
        const v = med(xs.filter(r => r.interval === tf).map(r => avgR(r[key])));
        h += `<div style="${color(v)};border-radius:7px;padding:7px 4px;text-align:center;font:650 12.5px var(--body);font-variant-numeric:tabular-nums">${v == null ? "–" : rf(v)}</div>`;
      }
    });
    return h + `</div>`;
  };
  const costs = d.study.costs;
  let key = "is", cost = 0;
  const render = () => {
    el.innerHTML = `<div class="seg">${["is", "oos"].map(k => `<button data-k="${k}" class="${k === key ? "on" : ""}">${k === "is" ? "Selection period" : "Afterwards"}</button>`).join("")}</div>${costs.length > 1 ? ` <div class="seg">${costs.map((c, i) => `<button data-c="${i}" class="${i === cost ? "on" : ""}">${esc(c.label)}</button>`).join("")}</div>` : ""}<div class="tw">${draw(key, cost)}</div>`;
    $$("[data-k]", el).forEach(b => b.onclick = () => { key = b.dataset.k; render(); });
    $$("[data-c]", el).forEach(b => b.onclick = () => { cost = +b.dataset.c; render(); });
  };
  render();
};

/** Median result of groups of variants, before and after, as paired bars. */
FIG.groups = async el => {
  const d = await load(el.dataset.study), cost = +(el.dataset.cost || 0);
  const by = el.dataset.by, where = el.dataset.where ? JSON.parse(el.dataset.where) : {};
  const keyOf = r => by === "family" ? r.name : by === "interval" ? r.interval : by === "interval+stop" ? `${r.interval === "1h" ? "hourly" : "daily"} entry · ${r.params.stop}` : (r.params[by] ?? "(the breakout itself)");
  const rows = rowsOf(d).filter(r => r.cost === cost && Object.entries(where).every(([k, v]) => k === "prefix" ? r.family.startsWith(v) : k === "notprefix" ? !r.family.startsWith(v) : r.params[k] === v));
  const g = new Map();
  for (const r of rows) { const k = keyOf(r); if (!g.has(k)) g.set(k, []); g.get(k).push(r); }
  const items = [...g.entries()].map(([k, rs]) => ({ k, n: rs.length, a: med(rs.map(r => avgR(r.is))), b: med(rs.map(r => avgR(r.oos))), gross: med(rs.map(r => avgR(r.is) != null && r.feesR != null ? avgR(r.is) + r.feesR : null)), tpd: med(rs.map(r => r.ptpd)) }));
  const showGross = el.dataset.gross === "1";
  const vals = items.flatMap(x => [x.a, x.b, showGross ? x.gross : 0]).filter(v => v != null);
  const lim = Math.max(0.2, ...vals.map(Math.abs)) * 1.6;
  const W = +(el.dataset.w || 720), rowH = showGross ? 44 : 36, H = 26 + items.length * rowH, L = W < 600 ? 180 : 250, Z = L + (W - L - 20) / 2;
  const X = v => Z + v / lim * (W - L - 20) / 2;
  let s = `<line x1="${Z}" x2="${Z}" y1="8" y2="${H - 4}" stroke="currentColor" opacity=".3"/>`;
  items.forEach((x, i) => {
    const y = 18 + i * rowH;
    s += text(0, y + 10, x.k, { size: 12.5, weight: 600 });
    s += text(0, y + 24, `${x.n} variants · ${fmt(x.tpd, 1)} trades a day`, { size: 10.5, opacity: 0.6 });
    const bar = (v, dy, col, lbl) => v == null ? "" : `<rect x="${Math.min(X(0), X(v))}" y="${y + dy}" width="${Math.max(1.5, Math.abs(X(v) - X(0)))}" height="9" rx="3" fill="${col}"/>${text(v >= 0 ? X(v) + 6 : X(v) - 6, y + dy + 8.5, `${rf(v)} ${lbl}`, { anchor: v >= 0 ? "start" : "end", size: 10.5, opacity: 0.85 })}`;
    s += bar(x.a, 0, "var(--accent)", "before") + bar(x.b, 12, "var(--up)", "after") + (showGross ? bar(x.gross, 24, "var(--faint)", "before, no costs") : "");
  });
  el.innerHTML = svg(W, H, s, "Median result by group") + `<div class="legend"><span><i style="background:var(--accent)"></i>selection period</span><span><i style="background:var(--up)"></i>afterwards</span>${showGross ? `<span><i style="background:var(--faint)"></i>selection period with costs removed</span>` : ""}<span>Median of each group's variants.</span></div>`;
};

/** Month-by-month bars, optionally against a benchmark line, from any recorded series. */
function monthBars(series, o = {}) {
  const n = series.length; if (!n) return "";
  const W = 900, H = 230, pad = 28, bh = o.benchmark || [];
  const hi = Math.max(1, ...series) * 1.08, lo = Math.min(-1, ...series) * 1.08;
  const Y = v => 12 + (hi - Math.max(lo, Math.min(hi, v))) / (hi - lo) * (H - pad - 12);
  const bw = (W - 50) / n, X = k => 46 + k * bw;
  let s = "";
  for (const v of [hi, 0, lo]) s += text(40, Y(v) + 4, (v > 0 ? "+" : "") + fmt(v, Math.abs(v) < 10 ? 1 : 0) + "%", { anchor: "end", opacity: 0.55, size: 10.5 });
  s += `<line x1="46" x2="${W}" y1="${Y(0)}" y2="${Y(0)}" stroke="currentColor" opacity=".25"/>`;
  if (o.split != null && o.split > 0 && o.split < n) s += `<rect x="${X(o.split)}" y="6" width="${W - X(o.split)}" height="${H - pad}" fill="var(--up-soft)"/>${text(X(o.split) + 6, 18, o.splitLabel || "held out", { size: 11, weight: 700, fill: "var(--up)" })}`;
  series.forEach((v, k) => { const y0 = Y(0), y1 = Y(v); s += `<rect x="${(X(k) + bw * 0.14).toFixed(1)}" y="${Math.min(y0, y1).toFixed(1)}" width="${Math.max(1, bw * 0.72).toFixed(1)}" height="${Math.max(1, Math.abs(y1 - y0)).toFixed(1)}" rx="1.5" fill="${v >= 0 ? "var(--up)" : "var(--down)"}"><title>${esc(monthName((o.month0 || 0) + k))}: ${pct(v)}</title></rect>`; });
  if (bh.length) s += `<polyline fill="none" stroke="var(--faint)" stroke-width="1.6" stroke-dasharray="3 2" points="${bh.slice(0, n).map((v, k) => `${(X(k) + bw / 2).toFixed(1)},${Y(v).toFixed(1)}`).join(" ")}"/>`;
  for (let k = 0; k < n; k++) { const m = (o.month0 || 0) + k; if (m % 12 === 0) s += text(X(k) + 2, H - 6, 1970 + Math.floor(m / 12), { opacity: 0.6, size: 10.5 }); }
  return svg(W, H, s, o.label || "Monthly results") + `<div class="legend"><span><i style="background:var(--up)"></i>month up</span><span><i style="background:var(--down)"></i>month down</span>${bh.length ? `<span><i style="background:none;border-top:2px dashed var(--faint);height:0;border-radius:0"></i>${esc(o.benchLabel || "buy and hold")} (cut off at the chart's edge)</span>` : ""}</div>`;
}
/** Growth of 1 from monthly % returns, with an optional benchmark. */
function equity(series, o = {}) {
  const grow = x => { let e = 1; return [1, ...x.map(v => e *= 1 + v / 100)]; };
  const a = grow(series), b = o.benchmark ? grow(o.benchmark.slice(0, series.length)) : null;
  const all = [...a, ...(b || [])].filter(v => v > 0), hi = Math.log(Math.max(...all)), lo = Math.log(Math.min(...all));
  const W = 900, H = 220, X = k => 46 + k / (a.length - 1) * (W - 56), Y = v => 12 + (hi - Math.log(Math.max(1e-6, v))) / (hi - lo || 1) * (H - 40);
  const line = (v, col, w, dash) => `<polyline fill="none" stroke="${col}" stroke-width="${w}" ${dash ? `stroke-dasharray="${dash}"` : ""} points="${v.map((x, k) => `${X(k).toFixed(1)},${Y(x).toFixed(1)}`).join(" ")}"/>`;
  let s = `<line x1="46" x2="${W}" y1="${Y(1)}" y2="${Y(1)}" stroke="currentColor" opacity=".25" stroke-dasharray="3 3"/>${text(40, Y(1) + 4, "1×", { anchor: "end", opacity: 0.6, size: 10.5 })}`;
  for (const v of [hi, lo]) if (Math.abs(Y(Math.exp(v)) - Y(1)) > 14) s += text(40, Y(Math.exp(v)) + 4, fmt(Math.exp(v), Math.exp(v) >= 10 ? 0 : 1) + "×", { anchor: "end", opacity: 0.6, size: 10.5 });
  if (b) s += line(b, "var(--faint)", 1.5, "4 3");
  s += line(a, "var(--accent)", 2.4);
  for (let k = 0; k < series.length; k++) { const m = (o.month0 || 0) + k; if (m % 12 === 0) s += text(X(k) + 2, H - 6, 1970 + Math.floor(m / 12), { opacity: 0.6, size: 10.5 }); }
  return svg(W, H, s, "Growth of the account") + `<div class="legend"><span><i style="background:var(--accent)"></i>${esc(o.label || "strategy")}, growth of 1 (log scale)</span>${b ? `<span><i style="background:var(--faint)"></i>${esc(o.benchLabel || "buy and hold")}</span>` : ""}</div>`;
}
function findRow(d, family, interval, cost, params, confirm) {
  return rowsOf(d).find(r => (confirm ? r.family.startsWith(family + "-") : r.family === family) && r.interval === interval && r.cost === cost && sameParams(r.params, params));
}

/** A study row's months (optionally joined with its confirmation run before it). */
FIG.months = async el => {
  const ds = el.dataset, params = JSON.parse(ds.params);
  const d = await load(ds.study), r = findRow(d, ds.family, ds.interval, +ds.cost, params);
  let series = r.monthly, bench = d.buy_hold_monthly, m0 = d.month0, split = monthOf(d.split_ts) - d.month0;
  if (ds.confirm) {
    const c = await load(ds.confirm), cr = findRow(c, ds.family, ds.interval, +ds.cost, params, true);
    const early = cr.monthly.slice(0, -1);
    series = [...early, ...r.monthly]; bench = [...c.buy_hold_monthly.slice(0, -1), ...d.buy_hold_monthly]; m0 = c.month0; split += early.length;
  }
  const st = mstats(series, true);
  el.innerHTML = `<div class="kpis" style="margin-bottom:14px"><div><b class="${cls(st.avg)}">${pct(st.avg, 2)}</b><span>average month, ${st.n} months</span></div><div><b>${st.up}/${st.n}</b><span>months that made money</span></div><div><b class="down">${pct(st.worst)}</b><span>worst month</span></div><div><b class="down">${pct(st.dd)}</b><span>largest fall from a peak</span></div></div>` +
    monthBars(series, { month0: m0, split, splitLabel: ds.splitLabel || "held out →" }) +
    `<div style="margin-top:14px">${equity(series, { benchmark: bench, month0: m0, label: "strategy", benchLabel: "buying and holding the same coins, rebalanced monthly" })}</div>`;
};

/** Fees as a multiple of the risk, by stop distance, with the timeframes' measured medians. */
FIG.fees = async el => {
  const d = await load("intraday-reversals"), d2 = await load("intraday-reversals-2");
  const cases = d.study.costs.map(c => ({ label: c.label, rt: 2 * (c.fee_bps + c.slippage_bps) / 100 }));
  const W = 720, H = 300, m = 50, xs = [0.1, 2], ys = [0, 2.5];
  const X = v => m + (Math.log(v) - Math.log(xs[0])) / (Math.log(xs[1]) - Math.log(xs[0])) * (W - m - 20);
  const Y = v => 12 + (ys[1] - Math.min(ys[1], v)) / (ys[1] - ys[0]) * (H - m - 12);
  let s = "";
  for (const v of [0.1, 0.2, 0.3, 0.5, 1, 2]) s += `<line x1="${X(v)}" x2="${X(v)}" y1="12" y2="${H - m}" stroke="currentColor" opacity=".08"/>${text(X(v), H - m + 16, v + "%", { anchor: "middle", opacity: 0.6, size: 10.5 })}`;
  for (const v of [0, 0.5, 1, 1.5, 2, 2.5]) s += `<line x1="${m}" x2="${W - 20}" y1="${Y(v)}" y2="${Y(v)}" stroke="currentColor" opacity=".08"/>${text(m - 6, Y(v) + 4, fmt(v, 1) + "R", { anchor: "end", opacity: 0.6, size: 10.5 })}`;
  s += `<rect x="${m}" y="${Y(0.1)}" width="${W - m - 20}" height="${Y(0) - Y(0.1)}" fill="var(--up-soft)"/>${text(m + 8, Y(0.1) - 5, "fees under 0.1R", { size: 10.5, fill: "var(--up)", weight: 700 })}`;
  const cols = ["var(--accent)", "var(--down)"];
  cases.forEach((c, i) => {
    const pts = []; for (let v = 0.1; v <= 2.0001; v *= 1.05) pts.push(`${X(v).toFixed(1)},${Y(c.rt / v).toFixed(1)}`);
    s += `<polyline fill="none" stroke="${cols[i]}" stroke-width="2.4" points="${pts.join(" ")}"/>`;
  });
  const byTf = {};
  for (const r of [...rowsOf(d), ...rowsOf(d2)]) if (r.cost === 0 && r.feesR != null && r.params.stop?.endsWith("%")) (byTf[r.params.stop] ||= []).push(r.feesR);
  for (const [stop, v] of Object.entries(byTf)) { const x = parseFloat(stop); s += `<circle cx="${X(x)}" cy="${Y(med(v))}" r="5" fill="var(--surface)" stroke="var(--accent)" stroke-width="2.5"><title>measured, ${stop} stop: ${fmt(med(v), 2)}R</title></circle>`; }
  s += text((W + m) / 2, H - 8, "Stop distance from entry (log scale)", { anchor: "middle", size: 12, weight: 600 });
  el.innerHTML = svg(W, H, s, "Fees per trade as a multiple of the risk") + `<div class="legend">${cases.map((c, i) => `<span><i style="background:${cols[i]}"></i>${esc(c.label)}: ${fmt(c.rt, 2)}% a round trip</span>`).join("")}<span><i style="background:var(--surface);border:2px solid var(--accent)"></i>measured in the study</span></div>`;
};

/** Study 1's stages, from the recorded files. */
FIG.funnel1 = async el => {
  const [a, b, c, e] = await Promise.all(["intraday-reversals", "intraday-reversals-2", "confirm-other-coins", "confirm-earlier-period"].map(load));
  const st = d => { const rows = rowsOf(d).filter(r => r.cost === 0); return { tests: d.rows.length * d.study.symbols.length, sel: rows.filter(r => r.selected).length, surv: rows.filter(r => r.survived).length, first: rows[0] }; };
  const A = st(a), B = st(b), C = st(c), E = st(e);
  const stages = [
    ["Round 1", `${A.tests.toLocaleString()} backtests`, "1m to 15m candles", ""],
    ["Selected", A.sel, "fees 0.3R to 1.4R a trade", "bad"],
    ["Round 2", `${B.tests.toLocaleString()} backtests`, "wider stops, 5m to 1h", ""],
    ["Selected → survived", `${B.sel} → ${B.surv}`, "one pin-bar rule", B.surv ? "good" : "bad"],
    ["Five other coins", rf(avgR(C.first.oos)), `profitable on ${C.first.upOos} of ${C.first.markets}`, avgR(C.first.oos) > 0 ? "good" : "bad"],
    ["The 90 days before", rf(avgR(E.first.oos)), `profitable on ${E.first.upOos} of ${E.first.markets}`, avgR(E.first.oos) > 0 ? "good" : "bad"],
  ];
  el.innerHTML = `<div class="grid3" style="grid-template-columns:repeat(auto-fit,minmax(max(200px,30%),1fr))">${stages.map(([t, n, p, k]) => `<div class="tile" style="${k === "bad" ? "background:var(--down-soft);border-color:rgba(229,72,77,.3)" : k === "good" ? "background:var(--up-soft);border-color:rgba(15,159,110,.3)" : ""}"><span style="font:700 11px var(--body);letter-spacing:.07em;text-transform:uppercase;color:var(--faint)">${esc(t)}</span><b style="font:800 21px var(--display)">${esc(String(n))}</b><p>${esc(p)}</p></div>`).join("")}</div>`;
};

/** Study 2's confirmation: every survivor on the three years before. */
FIG.confirm2 = async el => {
  const [o, c] = await Promise.all([load("swing-trend"), load("swing-trend-confirm")]);
  const conf = rowsOf(c), pts = [];
  for (const r of rowsOf(o).filter(r => r.survived)) {
    const cr = conf.find(x => x.family.startsWith(r.family + "-") && x.interval === r.interval && x.cost === r.cost && sameParams(x.params, r.params));
    if (cr) pts.push({ r, after: avgR(r.oos), before3: avgR(cr.oos), up: cr.upOos, n: cr.markets });
  }
  const W = 640, H = 360, m = 70, lim = Math.max(0.5, ...pts.map(p => p.after)) * 1.15;
  const X = v => m + Math.max(0, Math.min(lim, v)) / lim * (W - m - 16), yl = Math.max(0.5, ...pts.map(p => p.before3)) * 1.1, Y = v => 12 + (yl - Math.max(-0.2 * yl, Math.min(yl, v))) / (1.2 * yl) * (H - m - 12);
  let s = `<line x1="${m}" x2="${W - 16}" y1="${Y(0)}" y2="${Y(0)}" stroke="currentColor" opacity=".4"/>`;
  for (const v of [0, lim / 3, 2 * lim / 3, lim]) s += text(X(v), H - m + 16, rf(v), { anchor: "middle", opacity: 0.6, size: 10.5 });
  for (const v of [0, yl / 2, yl]) s += text(m - 6, Y(v) + 4, rf(v), { anchor: "end", opacity: 0.6, size: 10.5 });
  for (const p of pts) s += `<circle cx="${X(p.after)}" cy="${Y(p.before3)}" r="5" fill="${p.before3 > 0 ? "var(--up)" : "var(--down)"}" opacity=".8"><title>${esc(p.r.name)} · ${p.r.interval} · ${esc(Object.values(p.r.params).join(", "))}: ${rf(p.after)} held out, ${rf(p.before3)} on 2020–23 (${p.up}/${p.n} coins)</title></circle>`;
  s += text((W + m) / 2, H - 6, "Average per trade, held-out year (2025–26)", { anchor: "middle", size: 12, weight: 600 });
  s += `<text transform="translate(12 ${(H - m) / 2}) rotate(-90)" text-anchor="middle" font-size="12" font-weight="600" fill="currentColor">Average per trade, 2020–23 (never seen)</text>`;
  el.innerHTML = svg(W, H, s, "Survivors on the held-out year against the earlier, unseen years") + `<div class="legend"><span><i style="background:var(--up)"></i>${pts.filter(p => p.before3 > 0).length} of ${pts.length} survivors made money on 2020–23 too</span></div>`;
};

/** Portfolio studies: Sharpe before the split against after, for every variant. */
FIG.qscatter = async el => {
  const q = await load(el.dataset.quant);
  const W = +(el.dataset.w || 640), H = +(el.dataset.h || 380), m = 60, lo = -2, hi = 3;
  const X = v => m + (Math.max(lo, Math.min(hi, v)) - lo) / (hi - lo) * (W - m - 16), Y = v => 12 + (hi - Math.max(lo, Math.min(hi, v))) / (hi - lo) * (H - m - 12);
  let s = `<line x1="${m}" x2="${W - 16}" y1="${Y(0)}" y2="${Y(0)}" stroke="currentColor" opacity=".35"/><line x1="${X(0)}" x2="${X(0)}" y1="12" y2="${H - m}" stroke="currentColor" opacity=".35"/>`;
  s += `<line x1="${X(1)}" x2="${X(1)}" y1="12" y2="${H - m}" stroke="var(--accent)" stroke-dasharray="4 3" opacity=".7"/>${text(X(1) + 4, 22, "selection bar: Sharpe 1", { size: 10.5, fill: "var(--accent)", weight: 700 })}`;
  for (const v of [-2, -1, 0, 1, 2, 3]) { s += text(X(v), H - m + 16, fmt(v, 0), { anchor: "middle", opacity: 0.6, size: 10.5 }); s += text(m - 6, Y(v) + 4, fmt(v, 0), { anchor: "end", opacity: 0.6, size: 10.5 }); }
  for (const v of [...q.variants].sort((a, b) => a.selected - b.selected)) s += `<circle cx="${X(v.in_sample.sharpe).toFixed(1)}" cy="${Y(v.out_of_sample.sharpe).toFixed(1)}" r="${v.selected ? 3.8 : 2.6}" fill="${v.survived ? "var(--up)" : v.selected ? "var(--accent)" : "var(--faint)"}" opacity="${v.selected ? 0.9 : 0.4}"><title>${esc(Object.entries(v.params).map(([k, x]) => `${k} ${x}`).join(", "))}: ${fmt(v.in_sample.sharpe)} → ${fmt(v.out_of_sample.sharpe)}</title></circle>`;
  s += text((W + m) / 2, H - 6, "Sharpe ratio before the split", { anchor: "middle", size: 12, weight: 600 });
  s += `<text transform="translate(12 ${(H - m) / 2}) rotate(-90)" text-anchor="middle" font-size="12" font-weight="600" fill="currentColor">Sharpe ratio after the split</text>`;
  el.innerHTML = svg(W, H, s, "Sharpe before and after the split for every variant") + `<div class="legend"><span><i style="background:var(--faint)"></i>${q.variants.length} variants</span><span><i style="background:var(--accent)"></i>${q.variants.filter(v => v.selected).length} selected</span><span><i style="background:var(--up)"></i>${q.variants.filter(v => v.survived).length} survived</span><span>Values beyond −2 and 3 are drawn at the edge.</span></div>`;
};

/** One portfolio variant's months against its benchmark. */
FIG.qpick = async el => {
  const q = await load(el.dataset.quant), want = JSON.parse(el.dataset.params);
  const v = q.variants.find(x => sameParams(x.params, want)), o = v.out_of_sample, i = v.in_sample;
  el.innerHTML = `<div class="kpis" style="margin-bottom:14px"><div><b class="${cls(i.annual_pct)}">${pct(i.annual_pct, 0)}</b><span>a year before the split, Sharpe ${fmt(i.sharpe)}</span></div><div><b class="${cls(o.annual_pct)}">${pct(o.annual_pct, 0)}</b><span>a year after it, Sharpe ${fmt(o.sharpe)}</span></div><div><b>${pct(q.benchmark_out_of_sample.annual_pct, 0)}</b><span>${esc(q.benchmark_label)}, a year after</span></div><div><b class="down">${pct(o.max_drawdown_pct, 0)}</b><span>largest fall after the split</span></div></div>` +
    monthBars(v.monthly, { month0: q.month0, split: monthOf(q.split_ts) - q.month0, splitLabel: "after the split →" }) +
    `<div style="margin-top:14px">${equity(v.monthly, { benchmark: q.benchmark_monthly, month0: q.month0, label: "rotation", benchLabel: q.benchmark_label })}</div>`;
};

/** The combined trend and rotation portfolio. */
async function combo() {
  const [ix, sw, cf, rot, cr, ce] = await Promise.all(["index", "swing-trend", "swing-trend-confirm", "rotation", "carry-recent", "carry-earlier"].map(load));
  const toMap = (m0, a) => Object.fromEntries(a.map((v, i) => [m0 + i, v]));
  const pk = ix.picks[0];
  const main = findRow(sw, pk.family, pk.interval, pk.cost, pk.params), conf = findRow(cf, pk.family, pk.interval, pk.cost, pk.params, true);
  const trend = { ...toMap(cf.month0, conf.monthly.slice(0, -1)), ...toMap(sw.month0, main.monthly) };
  const rc = ix.combo.components[1], rv = rot.variants.find(v => sameParams(v.params, rc.params));
  const rotation = toMap(rot.month0, rv.monthly), btc = toMap(rot.month0, rot.benchmark_monthly);
  const carry = { ...toMap(ce.month0, ce.portfolio_always.slice(0, -1)), ...toMap(cr.month0, cr.portfolio_always) };
  const last = monthOf(Date.now()) - 1;
  const months = Object.keys(trend).map(Number).filter(m => m <= last && m in rotation && m in btc && m in carry).sort((a, b) => a - b);
  const [y, mo] = ix.combo.split.split("-").map(Number), split = (y - 1970) * 12 + mo - 1;
  const col = s => months.map(m => s[m] ?? 0), inIS = months.map(m => m < split);
  const vol = x => { const v = x.filter((_, i) => inIS[i]); const m = mean(v); return Math.sqrt(mean(v.map(a => (a - m) ** 2))); };
  const cols = [col(trend), col(rotation)], inv = cols.map(x => 1 / vol(x)), w = inv.map(v => v / (inv[0] + inv[1]));
  const mix = months.map((_, i) => w[0] * cols[0][i] + w[1] * cols[1][i]);
  return { months, split, w, trend: cols[0], rotation: cols[1], mix, btc: col(btc), carry: col(carry), inIS };
}
const corrOf = (x, y) => { const mx = mean(x), my = mean(y); let a = 0, b = 0, c = 0; for (let i = 0; i < x.length; i++) { a += (x[i] - mx) * (y[i] - my); b += (x[i] - mx) ** 2; c += (y[i] - my) ** 2; } return a / Math.sqrt(b * c); };
FIG.combo = async el => {
  const d = await combo();
  const line = (label, x) => { const a = mstats(x), o = mstats(x.filter((_, i) => !d.inIS[i])); return `<tr${label === "Combined" ? ` class="hl"` : ""}><td>${esc(label)}</td><td class="n ${cls(a.avg)}">${pct(a.avg, 2)}</td><td class="n down">${pct(a.worst)}</td><td class="n down">${pct(a.dd)}</td><td class="n">${a.up}/${a.n}</td><td class="n ${cls(o.avg)}">${pct(o.avg, 2)}</td><td class="n down">${pct(o.dd)}</td></tr>`; };
  el.innerHTML = `<div class="kpis" style="margin-bottom:14px"><div><b>${fmt(100 * d.w[0], 0)}% / ${fmt(100 * d.w[1], 0)}%</b><span>trend / rotation, fixed from data before ${monthName(d.split)}</span></div><div><b>${fmt(corrOf(d.trend, d.rotation), 2)}</b><span>monthly correlation between the two</span></div></div>
    <div class="tw"><table><thead><tr><th style="min-width:190px"></th><th class="n">Avg month</th><th class="n">Worst</th><th class="n">Largest fall</th><th class="n">Months up</th><th class="n">Avg, after</th><th class="n">Fall, after</th></tr></thead><tbody>
    ${line("Trend following (SuperTrend, 4h)", d.trend)}${line("Coin rotation (weekly)", d.rotation)}${line("Combined", d.mix)}${line("Funding carry", d.carry)}${line("Buying and holding BTC", d.btc)}</tbody></table></div>
    <div style="margin-top:16px">${monthBars(d.mix, { month0: d.months[0], split: d.months.indexOf(d.split), splitLabel: "weights fixed, held out →" })}</div>
    <div style="margin-top:14px">${equity(d.mix, { benchmark: d.btc, month0: d.months[0], label: "combined", benchLabel: "buying and holding BTC" })}</div>`;
};
/** The combined plan's growth against holding BTC, for the overview. */
FIG.growth = async el => {
  const d = await combo();
  el.innerHTML = equity(d.mix, { benchmark: d.btc, month0: d.months[0], label: "trend plus rotation (Study 3)", benchLabel: "buying and holding BTC" });
};
/** How returns and losses scale with position size, with 5,000 simulated years. */
FIG.risk = async el => {
  const d = await combo();
  const rng = seed => () => (seed = (seed * 1664525 + 1013904223) % 4294967296) / 4294967296;
  const sim = (x, k) => { const r = rng(7), out = []; for (let i = 0; i < 5000; i++) { let e = 1; for (let j = 0; j < 12; j++) e *= 1 + k * x[Math.floor(r() * x.length)] / 100; out.push(100 * (e - 1)); } return { med: quant(out, 0.5), p5: quant(out, 0.05), down: out.filter(v => v < 0).length / 50 }; };
  const rows = [0.5, 1, 1.5, 2, 3].map(k => { const m = mstats(d.mix.map(v => v * k)), s = sim(d.mix, k); return `<tr${k === 1 ? ` class="hl"` : ""}><td>${fmt(k, 1)}×</td><td class="n ${cls(m.avg)}">${pct(m.avg)}</td><td class="n down">${pct(m.worst)}</td><td class="n down">${pct(m.dd)}</td><td class="n">${pct(s.med, 0)}</td><td class="n">${pct(s.p5, 0)}</td><td class="n">${fmt(s.down, 0)}%</td></tr>`; }).join("");
  el.innerHTML = `<div class="tw"><table><thead><tr><th>Position size</th><th class="n">Average month</th><th class="n">Worst month</th><th class="n">Largest fall</th><th class="n">Typical year</th><th class="n">Bad year (worst 5%)</th><th class="n">Years that end down</th></tr></thead><tbody>${rows}</tbody></table></div>`;
};
FIG.carry = async el => {
  const c = await load(el.dataset.carry), yr = m => mean(m) * 12, st = mstats(c.portfolio_always);
  el.innerHTML = `<div class="kpis" style="margin-bottom:14px"><div><b class="${cls(st.avg)}">${pct(yr(c.portfolio_always), 1)}</b><span>a year, held all the time</span></div><div><b>${pct(yr(c.portfolio_filtered), 1)}</b><span>a year, only while funding was positive</span></div><div><b>${st.up}/${st.n}</b><span>months that made money</span></div><div><b class="down">${pct(st.dd)}</b><span>largest fall</span></div></div>` + monthBars(c.portfolio_always, { month0: c.month0 });
};

/** A portfolio replay: every risk level and position limit, and the chosen run's months. */
FIG.replay = async el => {
  const x = await load(el.dataset.replay);
  let cost = 0, pick = [+(el.dataset.risk || 0.5), +(el.dataset.open || 20)];
  const render = () => {
    const c = x.costs[cost], sel = c.runs.find(r => r.risk_pct === pick[0] && r.max_open === pick[1]) || c.runs[0];
    const rows = c.runs.map(r => `<tr class="${r === sel ? "hl" : ""}" data-p="${r.risk_pct},${r.max_open}" style="cursor:pointer"><td>${fmt(r.risk_pct, 1)}%</td><td class="n">${r.max_open}</td><td class="n ${cls(r.all.annual_pct)}">${pct(r.all.annual_pct)}</td><td class="n down">${pct(r.all.max_drawdown_pct)}</td><td class="n ${cls(r.after.annual_pct)}">${pct(r.after.annual_pct)}</td><td class="n down">${pct(r.after.max_drawdown_pct)}</td><td class="n">${r.all.taken}</td><td class="n">${r.all.skipped}</td></tr>`).join("");
    const h = sel.all.signals_per_day, days = h.reduce((a, b) => a + b, 0), perWeek = 7 * sel.all.signals / days;
    el.innerHTML = `<div class="seg">${x.costs.map((cc, i) => `<button data-c="${i}" class="${i === cost ? "on" : ""}">${esc(cc.label)}</button>`).join("")}</div>
      <div class="tw"><table><thead><tr><th>Risk per trade</th><th class="n">Max open</th><th class="n">A year</th><th class="n">Largest fall</th><th class="n">A year since ${day(x.split_ts)}</th><th class="n">Fall since</th><th class="n">Taken</th><th class="n">Skipped</th></tr></thead><tbody>${rows}</tbody></table></div>
      <p style="font-size:13.5px;color:var(--muted);margin:10px 0 14px">${c.trades} signals at ${rf(c.avg_r)} a trade on average. About ${fmt(perWeek, 1)} signals a week; ${fmt(100 * (days - h[0]) / days, 0)}% of days have at least one. Click a row to see its months.</p>
      ${monthBars(sel.all.monthly, { month0: sel.all.month0, split: monthOf(x.split_ts) - sel.all.month0, splitLabel: "never used to choose →" })}
      <div style="margin-top:14px">${equity(sel.all.monthly, { month0: sel.all.month0, label: `${fmt(sel.risk_pct, 1)}% a trade, at most ${sel.max_open} open` })}</div>`;
    $$("[data-c]", el).forEach(b => b.onclick = () => { cost = +b.dataset.c; render(); });
    $$("tr[data-p]", el).forEach(tr => tr.onclick = () => { pick = tr.dataset.p.split(",").map(Number); render(); });
  };
  render();
};

/** A real example: candles, the rule's lines and its trades, from the engine's own backtest. */
FIG.example = async el => {
  const ex = (await load("examples")).examples.find(e => e.id === el.dataset.example);
  if (!window.LightweightCharts) { el.innerHTML = `<p style="color:var(--muted)">Chart library not loaded.</p>`; return; }
  const box = document.createElement("div"); box.className = "chart"; el.appendChild(box);
  const chart = LightweightCharts.createChart(box, { autoSize: true, layout: { background: { color: css("--surface") }, textColor: css("--muted"), fontFamily: "Inter, sans-serif", fontSize: 11 },
    grid: { vertLines: { visible: false }, horzLines: { color: css("--line") } }, rightPriceScale: { borderVisible: false }, timeScale: { borderVisible: false, timeVisible: ex.interval !== "1d" }, handleScroll: false, handleScale: false });
  const t = ms => Math.floor(ms / 1000);
  const series = chart.addCandlestickSeries({ upColor: css("--up"), downColor: css("--down"), wickUpColor: css("--up"), wickDownColor: css("--down"), borderVisible: false });
  series.setData(ex.candles.map(c => ({ time: t(c[0]), open: c[1], high: c[2], low: c[3], close: c[4] })));
  const colors = [css("--accent"), "#f59e0b", css("--accent2")];
  ex.lines.forEach((l, i) => { const s = chart.addLineSeries({ color: colors[i % 3], lineWidth: 2, priceLineVisible: false, lastValueVisible: false, crosshairMarkerVisible: false }); s.setData(ex.candles.map((c, k) => l.values[k] == null ? { time: t(c[0]) } : { time: t(c[0]), value: l.values[k] })); });
  const marks = [];
  for (const tr of ex.trades) {
    const long = tr.side === "long";
    marks.push({ time: t(tr.entry_ts), position: long ? "belowBar" : "aboveBar", color: long ? css("--up") : css("--down"), shape: long ? "arrowUp" : "arrowDown", text: long ? "buy" : "sell" });
    if (tr.exit_ts <= ex.candles[ex.candles.length - 1][0]) marks.push({ time: t(tr.exit_ts), position: long ? "aboveBar" : "belowBar", color: tr.r >= 0 ? css("--up") : css("--down"), shape: "circle", text: rf(tr.r) });
  }
  series.setMarkers(marks.sort((a, b) => a.time - b.time));
  chart.timeScale().fitContent();
  const wins = ex.trades.filter(x => x.r > 0).length;
  const cap = document.createElement("p"); cap.className = "fs"; cap.style.margin = "10px 0 0";
  cap.innerHTML = `${ex.trades.length} trades in this window, ${wins} winners: ${ex.trades.map(x => `<span class="${cls(x.r)}">${rf(x.r)}</span>`).join(" · ")}. Every trade shown, not a selection.`;
  el.appendChild(cap);
};
/** The weekly candles of the weekly example, with the engulfing weeks marked. */
FIG.weeks = async el => {
  const ex = (await load("examples")).examples.find(e => e.id === el.dataset.example);
  const w = ex.weekly, bars = w.map(c => [c[1], c[2], c[3], c[4]]);
  const marks = [];
  for (let i = 1; i < w.length; i++) {
    const [, o1, , , c1] = w[i - 1], [, o, , , c] = w[i];
    if (c1 < o1 && c > o && o <= c1 && c >= o1 && Math.abs(c - o) > Math.abs(c1 - o1)) marks.push({ i, y: w[i][3], up: true, label: "engulfing" });
  }
  el.innerHTML = candles(bars, { w: 900, h: 220, marks, x0: 10, label: "Weekly candles" });
};

/** A 24-hour UTC day with the London and New York opens, their ranges and trading windows. */
FIG.sessions = el => {
  const W = 900, H = 150, X = h => 30 + h / 24 * (W - 60);
  let s = "";
  for (let h = 0; h <= 24; h += 3) s += `<line x1="${X(h)}" x2="${X(h)}" y1="20" y2="${H - 22}" stroke="currentColor" opacity=".1"/>${text(X(h), H - 6, String(h).padStart(2, "0") + ":00", { anchor: "middle", opacity: 0.6, size: 10.5 })}`;
  const row = (y, name, open, col) => `${text(X(0), y - 6, name, { size: 12, weight: 700 })}<rect x="${X(open)}" y="${y}" width="${X(open + 0.5) - X(open)}" height="22" rx="4" fill="${col}"/>${text(X(open + 0.25), y + 15, "range", { anchor: "middle", fill: "#fff", size: 10, weight: 700 })}<rect x="${X(open + 0.5)}" y="${y}" width="${X(open + 4.5) - X(open + 0.5)}" height="22" rx="4" fill="${col}" opacity=".25"/>${text(X(open + 2.5), y + 15, "trade window: 1, 2 or 4 hours", { anchor: "middle", size: 10.5, weight: 600 })}`;
  s += row(40, "London 08:00 local (07:00 UTC in summer, 08:00 in winter)", 7, "var(--accent)");
  s += row(92, "New York 09:30 local (13:30 UTC in summer, 14:30 in winter)", 13.5, "var(--up)");
  el.innerHTML = svg(W, H, s, "Session windows on a UTC day");
};

async function draw() {
  for (const el of $$("[data-fig]")) {
    try { await FIG[el.dataset.fig](el); } catch (e) { el.innerHTML = `<p style="color:var(--muted)">This figure couldn't load (${esc(e.message)}). Open the page through a web server, not from a file.</p>`; }
  }
  // Values some pages print in their text.
  for (const el of $$("[data-val]")) el.textContent = el.dataset.val;
}

/* ---------- page chrome ---------- */
function toc() {
  const toc = document.querySelector(".toc"); if (!toc) return;
  const secs = $$("article > section[id]");
  toc.innerHTML = `<b>On this page</b>` + secs.map(s => `<a href="#${s.id}">${esc(s.dataset.toc || [...(s.querySelector("h2")?.childNodes || [])].filter(n => !n.classList?.contains("n")).map(n => n.textContent).join("").trim() || s.id)}</a>`).join("");
  const links = $$("a", toc);
  const io = new IntersectionObserver(es => es.forEach(e => { if (e.isIntersecting) links.forEach(a => a.classList.toggle("on", a.getAttribute("href") === "#" + e.target.id)); }), { rootMargin: "-40% 0px -55% 0px" });
  secs.forEach(s => io.observe(s));
}
document.addEventListener("DOMContentLoaded", () => { toc(); draw(); });
})();
