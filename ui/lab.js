"use strict";
/*
 * Chart Lab: select candles on a chart, turn their shape into rules, see every
 * place the same thing happened and what followed, then backtest it.
 *
 * Matching and backtests run on the server with the same engine as every other
 * test, so what the lab shows is exactly what a strategy file would do. Swing
 * lines, higher-timeframe levels and the pattern names shown for a selection
 * are drawn here, using the same definitions as the engine.
 */
(() => {
const RANGES = [["1W", 7], ["1M", 30], ["3M", 91], ["6M", 182], ["1Y", 365], ["3Y", 1095], ["5Y", 1826]];
const MINUTES = { "1m": 1, "5m": 5, "15m": 15, "30m": 30, "1h": 60, "4h": 240, "1d": 1440, "1w": 10080 };
const MAX_BARS = 60000;
const COINS = ["BTCUSDT", "ETHUSDT", "SOLUSDT", "BNBUSDT", "XRPUSDT"];
const TIMEFRAMES = ["5m", "15m", "1h", "4h", "1d"];
const HINTS = {
  move: "Drag to scroll, wheel to zoom. Press <span class=\"kbd\">S</span> to select candles.",
  select: "Drag across 1 to 5 candles to use them as a setup.",
  mark: "Click above a candle to mark a swing high, below it for a swing low. Click again to remove.",
  level: "Click to draw a level. Drag a level to move it; double-click it to delete.",
};

const L = {
  candles: [], byTs: new Map(), symbol: "", interval: "1h", csv: null, csvName: "",
  tf: "1h", range: "6M", tool: "move",
  chart: null, series: null, vol: null, zz: null, perHi: null, perLo: null,
  sel: null, drag: null,
  marks: [], levels: [], levelSeq: 0,
  pivots: [], state: null,
  setup: null, dir: "long", stop: "setup", target: "2", matches: [], cur: -1,
  patternHits: [], trades: [], scanSeq: 0, scanTimer: null,
};

/* ---------- helpers ---------- */
const hex2rgba = (h, a) => {
  h = h.trim();
  if (h.startsWith("rgb")) return h.replace(/rgba?\(([^)]+)\)/, (_, v) => `rgba(${v.split(",").slice(0, 3).join(",")},${a})`);
  const n = parseInt(h.slice(1).length === 3 ? h.slice(1).replace(/./g, "$&$&") : h.slice(1), 16);
  return `rgba(${n >> 16 & 255},${n >> 8 & 255},${n & 255},${a})`;
};
const priceFmt = p => p == null ? "–" : p >= 1000 ? fmt(p, 1) : p >= 10 ? fmt(p, 2) : p >= 1 ? fmt(p, 3) : fmt(p, 5);
const sec = ms => Math.floor(ms / 1000);
const parts = c => ({ body: Math.abs(c.close - c.open), range: c.high - c.low, upper: c.high - Math.max(c.open, c.close), lower: Math.min(c.open, c.close) - c.low });
const ref = (name, k) => k > 0 ? `${name}[${k}]` : name;
const scaled = (x, name, k) => `${x} * ${ref(name, k)}`;
const signPct = v => (v > 0 ? "+" : v < 0 ? "−" : "") + fmt(Math.abs(v * 100), 2) + "%";
const cls = v => v > 0 ? "up" : v < 0 ? "down" : "";
function dataReq() {
  const c = L.candles;
  return { symbol: L.symbol, interval: L.interval, from: String(c[0].ts), to: String(c[c.length - 1].ts + 1), csv: L.csv };
}

/* ---------- the same swing definition as the engine ---------- */
function swings(c, left, right) {
  const piv = [], n = c.length;
  const st = { res: new Array(n).fill(null), sup: new Array(n).fill(null), structure: new Array(n).fill(0) };
  let res = null, sup = null, pres = null, psup = null;
  for (let e = 0; e < n; e++) {
    const p = e - right;
    if (p - left >= 0) {
      let hi = true, lo = true;
      for (let j = p - left; j < p; j++) { if (!(c[p].high > c[j].high)) hi = false; if (!(c[p].low < c[j].low)) lo = false; }
      for (let j = p + 1; j <= e; j++) { if (!(c[p].high >= c[j].high)) hi = false; if (!(c[p].low <= c[j].low)) lo = false; }
      if (hi) { pres = res; res = c[p].high; piv.push({ i: p, type: "H", price: c[p].high }); }
      if (lo) { psup = sup; sup = c[p].low; piv.push({ i: p, type: "L", price: c[p].low }); }
    }
    st.res[e] = res; st.sup[e] = sup;
    st.structure[e] = res != null && sup != null && pres != null && psup != null ? (res > pres && sup > psup ? 1 : res < pres && sup < psup ? -1 : 0) : 0;
  }
  return { pivots: piv, state: st };
}
/** Alternating highs and lows, keeping the more extreme of any run, labelled HH/LH/HL/LL. */
function zigzag(pivots) {
  const out = [];
  for (const p of [...pivots].sort((a, b) => a.i - b.i || (a.type === "H" ? -1 : 1))) {
    const last = out[out.length - 1];
    if (last && last.type === p.type) {
      if (p.type === "H" ? p.price > last.price : p.price < last.price) out[out.length - 1] = p;
    } else out.push(p);
  }
  let ph = null, pl = null;
  for (const p of out) {
    if (p.type === "H") { p.label = ph == null ? "H" : p.price > ph ? "HH" : "LH"; ph = p.price; }
    else { p.label = pl == null ? "L" : p.price > pl ? "HL" : "LL"; pl = p.price; }
  }
  return out;
}
function monthIndex(ts) { const d = new Date(ts); return d.getUTCFullYear() * 12 + d.getUTCMonth(); }
function periodKey(ts, minutes) {
  if (minutes === 43200) return monthIndex(ts);
  if (minutes === 10080) return Math.floor((Math.floor(ts / 86400000) - 4) / 7);
  return Math.floor(ts / (minutes * 60000));
}
/** Previous completed period's high and low at every candle. */
function periodLevels(c, minutes) {
  const hi = [], lo = []; let key = null, cur = null, prev = null;
  for (const b of c) {
    const k = periodKey(b.ts, minutes);
    if (k !== key) { key = k; prev = cur; cur = { h: b.high, l: b.low }; }
    else { cur.h = Math.max(cur.h, b.high); cur.l = Math.min(cur.l, b.low); }
    hi.push(prev ? prev.h : null); lo.push(prev ? prev.l : null);
  }
  return { hi, lo };
}
/** Named patterns completed at candle i, with the engine's definitions and default settings. */
function patternsAt(c, i) {
  const out = [], c0 = c[i], p0 = parts(c0);
  const bull = x => x.close > x.open, bear = x => x.close < x.open;
  if (p0.range > 0) {
    const small = Math.max(p0.body, 0.05 * p0.range);
    if (p0.lower >= 2 * small && p0.upper <= 0.25 * p0.range) out.push("hammer");
    if (p0.upper >= 2 * small && p0.lower <= 0.25 * p0.range) out.push("shooting_star");
    if (p0.body <= 0.1 * p0.range) out.push("doji");
    if (bull(c0) && p0.body >= 0.9 * p0.range) out.push("bullish_marubozu");
    if (bear(c0) && p0.body >= 0.9 * p0.range) out.push("bearish_marubozu");
  }
  if (i >= 1) {
    const c1 = c[i - 1], b1 = Math.abs(c1.close - c1.open);
    if (bear(c1) && bull(c0) && c0.open <= c1.close && c0.close >= c1.open && p0.body > b1) out.push("bullish_engulfing");
    if (bull(c1) && bear(c0) && c0.open >= c1.close && c0.close <= c1.open && p0.body > b1) out.push("bearish_engulfing");
    if (c0.high <= c1.high && c0.low >= c1.low && (c0.high < c1.high || c0.low > c1.low)) out.push("inside_bar");
    if (c0.high >= c1.high && c0.low <= c1.low && (c0.high > c1.high || c0.low < c1.low)) out.push("outside_bar");
  }
  if (i >= 2) {
    const c2 = c[i - 2], c1 = c[i - 1], p2 = parts(c2), p1 = parts(c1), mid2 = (c2.open + c2.close) / 2;
    if (bear(c2) && p2.body >= 0.5 * p2.range && p1.body <= 0.5 * p2.body && bull(c0) && c0.close > mid2) out.push("morning_star");
    if (bull(c2) && p2.body >= 0.5 * p2.range && p1.body <= 0.5 * p2.body && bear(c0) && c0.close < mid2) out.push("evening_star");
    const strong = p => p.range > 0 && p.body >= 0.5 * p.range;
    if (bull(c2) && bull(c1) && bull(c0) && c1.close > c2.close && c0.close > c1.close && c1.open > c2.open && c1.open <= c2.close && c0.open > c1.open && c0.open <= c1.close && strong(p2) && strong(p1) && strong(p0)) out.push("three_white_soldiers");
    if (bear(c2) && bear(c1) && bear(c0) && c1.close < c2.close && c0.close < c1.close && c1.open < c2.open && c1.open >= c2.close && c0.open < c1.open && c0.open >= c1.close && strong(p2) && strong(p1) && strong(p0)) out.push("three_black_crows");
  }
  return out;
}
const PATTERN_NAME = Object.fromEntries(PATTERNS.map(p => [p[0], p[1]]));

/* ---------- chart ---------- */
function chartOptions() {
  return {
    autoSize: true,
    layout: { background: { color: css("--surface") }, textColor: css("--muted"), fontFamily: "Inter, system-ui, sans-serif", fontSize: 11 },
    grid: { vertLines: { visible: false }, horzLines: { color: hex2rgba(css("--line"), 0.7) } },
    rightPriceScale: { borderVisible: false, scaleMargins: { top: 0.08, bottom: 0.18 } },
    timeScale: { borderVisible: false, timeVisible: true, rightOffset: 8, minBarSpacing: 0.5 },
    crosshair: { mode: 0, vertLine: { color: hex2rgba(css("--accent"), 0.45), labelBackgroundColor: css("--accent") }, horzLine: { color: hex2rgba(css("--accent"), 0.45), labelBackgroundColor: css("--accent") } },
  };
}
function seriesColors() {
  return { upColor: css("--up"), downColor: css("--down"), wickUpColor: css("--up"), wickDownColor: css("--down"), borderVisible: false };
}
function ensureChart() {
  if (L.chart) return;
  L.chart = LightweightCharts.createChart($("lab-chart"), chartOptions());
  L.series = L.chart.addCandlestickSeries(seriesColors());
  L.vol = L.chart.addHistogramSeries({ priceScaleId: "vol", priceFormat: { type: "volume" }, lastValueVisible: false, priceLineVisible: false });
  L.chart.priceScale("vol").applyOptions({ scaleMargins: { top: 0.84, bottom: 0 } });
  const line = { priceLineVisible: false, lastValueVisible: false, crosshairMarkerVisible: false };
  L.perHi = L.chart.addLineSeries({ ...line, lineWidth: 1, lineStyle: 2, lineType: 1 });
  L.perLo = L.chart.addLineSeries({ ...line, lineWidth: 1, lineStyle: 2, lineType: 1 });
  L.zz = L.chart.addLineSeries({ ...line, lineWidth: 2 });
  lineColors();
  L.chart.timeScale().subscribeVisibleLogicalRangeChange(drawSel);
  L.chart.subscribeCrosshairMove(p => crosshair(p));
  new ResizeObserver(drawSel).observe($("lab-wrap"));
  bindPointer();
  setTool(L.tool);
}
function lineColors() {
  L.zz.applyOptions({ color: hex2rgba(css("--accent2"), 0.85) });
  L.perHi.applyOptions({ color: hex2rgba(css("--down"), 0.65) });
  L.perLo.applyOptions({ color: hex2rgba(css("--up"), 0.65) });
  L.levels.forEach(l => l.line.applyOptions({ color: css("--accent2") }));
}
window.labTheme = () => {
  if (!L.chart) return;
  L.chart.applyOptions(chartOptions()); L.series.applyOptions(seriesColors()); lineColors(); drawVolume(); refreshMarkers();
};
window.labShown = () => {
  if (!L.candles.length && !L.loading) loadChart();
  requestAnimationFrame(drawSel);
};

function drawVolume() {
  if (!L.vol) return;
  const on = $("lab-vol").checked, up = hex2rgba(css("--up"), 0.28), dn = hex2rgba(css("--down"), 0.28);
  L.vol.setData(on ? L.candles.map(c => ({ time: sec(c.ts), value: c.volume, color: c.close >= c.open ? up : dn })) : []);
}
function drawSwings() {
  const sw = swings(L.candles, +$("lab-left").value || 3, +$("lab-right").value || 3);
  L.state = sw.state;
  L.pivots = $("lab-zz").checked ? zigzag(sw.pivots) : [];
  L.zz.setData(L.pivots.map(p => ({ time: sec(L.candles[p.i].ts), value: p.price })));
}
function drawPeriod() {
  const m = +$("lab-period").value;
  if (!m) { L.perHi.setData([]); L.perLo.setData([]); return; }
  const { hi, lo } = periodLevels(L.candles, m);
  const pts = arr => L.candles.map((c, i) => arr[i] == null ? { time: sec(c.ts) } : { time: sec(c.ts), value: arr[i] });
  L.perHi.setData(pts(hi)); L.perLo.setData(pts(lo));
}
function refreshMarkers() {
  if (!L.series) return;
  const c = L.candles, out = [], acc = css("--accent"), acc2 = css("--accent2"), faint = css("--faint");
  for (const p of L.pivots) out.push({ time: sec(c[p.i].ts), position: p.type === "H" ? "aboveBar" : "belowBar", color: faint, shape: "circle", size: 0.1, text: p.label });
  for (const i of L.patternHits) out.push({ time: sec(c[i].ts), position: "aboveBar", color: css("--warn"), shape: "square", size: 0.6, text: "" });
  const long = L.dir === "long";
  L.matches.forEach((i, j) => out.push({ time: sec(c[i].ts), position: long ? "belowBar" : "aboveBar", color: acc, shape: long ? "arrowUp" : "arrowDown", text: j === L.cur ? `${j + 1}/${L.matches.length}` : "" }));
  for (const t of L.trades) {
    const i = L.byTs.get(t.exit_ts); if (i == null) continue;
    const tag = { stop_loss: "SL", take_profit: "TP", trailing_stop: "TS", max_bars: "time", risk_limit: "limit", end_of_data: "end" }[t.reason] || "";
    out.push({ time: sec(t.exit_ts), position: long ? "aboveBar" : "belowBar", color: t.pnl >= 0 ? css("--up") : css("--down"), shape: "circle", size: 0.8, text: tag });
  }
  for (const m of L.marks) out.push({ time: sec(c[m.i].ts), position: m.type === "H" ? "aboveBar" : "belowBar", color: acc2, shape: m.type === "H" ? "arrowDown" : "arrowUp", size: 1.4, text: m.type === "H" ? "you: high" : "you: low" });
  out.sort((a, b) => a.time - b.time);
  L.series.setMarkers(out);
}

/* ---------- loading ---------- */
function renderMarketControls() {
  $("lab-tf").innerHTML = CAT.intervals.map(i => `<button data-tf="${i}" class="${i === L.tf ? "on" : ""}">${i}</button>`).join("");
  $("lab-tf").querySelectorAll("button").forEach(b => b.onclick = () => { L.tf = b.dataset.tf; renderMarketControls(); loadChart(); });
  const per = MINUTES[L.tf];
  $("lab-range").innerHTML = RANGES.map(([k, d]) => {
    const tooBig = d * 1440 / per > MAX_BARS;
    return `<button data-r="${k}" class="${k === L.range ? "on" : ""}" ${tooBig ? `disabled title="Too many ${L.tf} candles; pick a shorter period"` : ""}>${k}</button>`;
  }).join("");
  if ($("lab-range").querySelector(`[data-r="${L.range}"]`).disabled) {
    L.range = [...RANGES].reverse().find(([, d]) => d * 1440 / per <= MAX_BARS)[0];
    return renderMarketControls();
  }
  $("lab-range").querySelectorAll("button").forEach(b => b.onclick = () => { L.range = b.dataset.r; renderMarketControls(); loadChart(); });
  const po = $("lab-pattern");
  if (po.options.length === 1) po.innerHTML += PATTERNS.map(p => `<option value="${p[0]}">${esc(p[1])}</option>`).join("");
}
async function loadChart() {
  ensureChart();
  const btn = $("lab-load"), old = btn.textContent;
  btn.disabled = true; btn.innerHTML = `<span class="spinner"></span> Loading`;
  L.loading = true;
  const days = RANGES.find(r => r[0] === L.range)[1];
  const from = new Date(Date.now() - days * 86400000).toISOString().slice(0, 10);
  try {
    const r = await api("/api/candles", { symbol: $("lab-symbol").value.trim().toUpperCase() || "BTCUSDT", interval: L.tf, from, csv: L.csv });
    if (!r.ok) { hint(`<span style="color:var(--down)">${esc(r.errors.join(" "))}</span>`); return; }
    L.candles = r.candles; L.symbol = r.symbol; L.interval = r.interval;
    L.byTs = new Map(L.candles.map((c, i) => [c.ts, i]));
    L.sel = null; L.marks = []; L.trades = []; L.matches = []; L.cur = -1; L.patternHits = [];
    $("lab-empty").hidden = true;
    L.series.setData(L.candles.map(c => ({ time: sec(c.ts), open: c.open, high: c.high, low: c.low, close: c.close })));
    drawVolume(); drawSwings(); drawPeriod();
    const n = L.candles.length;
    L.chart.timeScale().setVisibleLogicalRange({ from: Math.max(0, n - 160), to: n + 8 });
    drawSel(); renderMarks(); renderLevels(); foot();
    if ($("lab-pattern").value) scanPattern();
    $("lab-bt-out").innerHTML = ""; $("lab-cmp").innerHTML = "";
    if (L.setup) { renderRules(); rescan(); } else refreshMarkers();
    setTool(L.tool);
  } catch (e) {
    hint(`<span style="color:var(--down)">Couldn't reach candlerail. Is it still running?</span>`);
  } finally { btn.disabled = false; btn.textContent = old; L.loading = false; }
}
function foot(extra = "") {
  const c = L.candles; if (!c.length) return;
  $("lab-foot").innerHTML = `<b>${esc(L.symbol)} · ${esc(L.interval)}</b><span>${c.length.toLocaleString()} candles, ${dt(c[0].ts).slice(0, 10)} to ${dt(c[c.length - 1].ts).slice(0, 10)}</span>
    <span><i style="background:${css("--accent")}"></i>matches</span><span><i style="background:${css("--accent2")}"></i>swings and your marks</span>
    ${$("lab-period").value ? `<span><i style="background:${hex2rgba(css("--down"), .65)}"></i>previous high <i style="background:${hex2rgba(css("--up"), .65)};margin-left:6px"></i>low</span>` : ""}
    <span class="spacer"></span><span id="lab-ohlc">${extra}</span>`;
}
function crosshair(p) {
  const el = $("lab-ohlc"); if (!el) return;
  if (!p || p.time == null) { el.textContent = ""; return; }
  const i = L.byTs.get(p.time * 1000); if (i == null) return;
  const c = L.candles[i], q = parts(c), pc = x => q.range > 0 ? Math.round(x / q.range * 100) + "%" : "–";
  el.innerHTML = `O ${priceFmt(c.open)} H ${priceFmt(c.high)} L ${priceFmt(c.low)} C <b class="${c.close >= c.open ? "up" : "down"}">${priceFmt(c.close)}</b> · body ${pc(q.body)} · wicks ${pc(q.upper)} / ${pc(q.lower)}`;
}
function hint(html) { $("lab-hint").innerHTML = html || HINTS[L.tool]; }

/* ---------- tools ---------- */
function setTool(t) {
  L.tool = t;
  $("lab-tool").querySelectorAll("button").forEach(b => b.classList.toggle("on", b.dataset.tool === t));
  $("lab-wrap").className = "lab-chart-wrap tool-" + t;
  const move = t === "move";
  L.chart?.applyOptions({ handleScroll: { mouseWheel: true, pressedMouseMove: move, horzTouchDrag: move, vertTouchDrag: false }, handleScale: { mouseWheel: true, pinch: true, axisPressedMouseMove: true } });
  hint();
}
$("lab-tool").querySelectorAll("button").forEach(b => b.onclick = () => setTool(b.dataset.tool));
function barAt(x) {
  const l = L.chart.timeScale().coordinateToLogical(x);
  if (l == null || !L.candles.length) return null;
  return Math.max(0, Math.min(L.candles.length - 1, Math.round(l)));
}
function bindPointer() {
  const el = $("lab-chart");
  const pos = e => { const r = el.getBoundingClientRect(); return { x: e.clientX - r.left, y: e.clientY - r.top }; };
  const nearLevel = y => L.levels.find(l => Math.abs((L.series.priceToCoordinate(l.price) ?? -99) - y) <= 6);
  el.addEventListener("pointerdown", e => {
    if (L.tool === "move" || !L.candles.length || e.button !== 0) return;
    const { x, y } = pos(e), i = barAt(x); if (i == null) return;
    if (L.tool === "select") { L.drag = { kind: "select", a: i }; L.sel = { a: i, b: i }; drawSel(); }
    else if (L.tool === "mark") toggleMark(i, L.series.coordinateToPrice(y));
    else if (L.tool === "level") {
      let lv = nearLevel(y);
      if (!lv) lv = addLevel(L.series.coordinateToPrice(y));
      L.drag = { kind: "level", lv };
    }
    el.setPointerCapture(e.pointerId);
  });
  el.addEventListener("pointermove", e => {
    const { x, y } = pos(e);
    if (L.tool === "level" && !L.drag) el.style.cursor = nearLevel(y) ? "ns-resize" : "";
    if (!L.drag) return;
    if (L.drag.kind === "select") {
      let b = barAt(x); if (b == null) return;
      b = Math.max(L.drag.a - 4, Math.min(L.drag.a + 4, b));
      L.sel = { a: L.drag.a, b }; drawSel();
    } else if (L.drag.kind === "level") {
      const p = L.series.coordinateToPrice(y); if (p == null) return;
      L.drag.lv.price = p; L.drag.lv.line.applyOptions({ price: p });
    }
  });
  const end = () => {
    const d = L.drag; L.drag = null; if (!d) return;
    if (d.kind === "select") makeSetup(Math.min(L.sel.a, L.sel.b), Math.max(L.sel.a, L.sel.b));
    if (d.kind === "level") { renderLevels(); levelsChanged(); }
  };
  el.addEventListener("pointerup", end);
  el.addEventListener("pointercancel", end);
  el.addEventListener("dblclick", e => {
    if (L.tool !== "level") return;
    const lv = nearLevel(pos(e).y); if (lv) removeLevel(lv.id);
  });
}
function drawSel() {
  const box = $("lab-sel");
  if (!L.sel || !L.chart) { box.hidden = true; return; }
  const ts = L.chart.timeScale(), a = Math.min(L.sel.a, L.sel.b), b = Math.max(L.sel.a, L.sel.b);
  const xa = ts.logicalToCoordinate(a), xb = ts.logicalToCoordinate(b), step = (ts.logicalToCoordinate(a + 1) ?? xa) - xa;
  if (xa == null || xb == null) { box.hidden = true; return; }
  const half = Math.max(3, step / 2);
  box.hidden = false; box.style.left = (xa - half) + "px"; box.style.width = (xb - xa + 2 * half) + "px";
  box.querySelector("span").textContent = `${b - a + 1} candle${b > a ? "s" : ""}`;
}
document.addEventListener("keydown", e => {
  if (activeTab !== "lab" || e.metaKey || e.ctrlKey || e.altKey || /INPUT|TEXTAREA|SELECT/.test(document.activeElement?.tagName)) return;
  const t = { v: "move", s: "select", m: "mark", l: "level" }[e.key.toLowerCase()];
  if (t) { setTool(t); e.preventDefault(); }
  if (e.key === "Escape") { L.sel = null; drawSel(); }
  if (e.key === "ArrowRight" && L.matches.length) { step(1); e.preventDefault(); }
  if (e.key === "ArrowLeft" && L.matches.length) { step(-1); e.preventDefault(); }
});

/* ---------- swing marks ---------- */
function toggleMark(i, price) {
  const c = L.candles[i], type = price >= (c.high + c.low) / 2 ? "H" : "L";
  const at = L.marks.findIndex(m => m.i === i && m.type === type);
  if (at >= 0) L.marks.splice(at, 1); else L.marks.push({ i, type });
  refreshMarkers(); renderMarks();
}
/** The swing setting whose swings best agree with the user's marks. */
function fitMarks() {
  const marks = L.marks; if (marks.length < 3) return null;
  const lo = Math.min(...marks.map(m => m.i)) - 2, hi = Math.max(...marks.map(m => m.i)) + 2;
  let best = null;
  for (let right = 1; right <= 10; right++) for (let left = 1; left <= 10; left++) {
    const piv = swings(L.candles, left, right).pivots.filter(p => p.i >= lo && p.i <= hi);
    const found = marks.filter(m => piv.some(p => p.type === m.type && Math.abs(p.i - m.i) <= 1)).length;
    const extra = piv.filter(p => !marks.some(m => m.type === p.type && Math.abs(p.i - m.i) <= 1)).length;
    const f1 = 2 * found / (2 * found + (marks.length - found) + extra);
    if (!best || f1 > best.f1 + 1e-9) best = { left, right, found, extra, f1 };
  }
  return best;
}
function renderMarks() {
  $("lab-marks").hidden = !L.marks.length;
  if (!L.marks.length) return;
  const b = fitMarks(), n = L.marks.length;
  $("lab-marks-out").innerHTML = !b ? `${n} mark${n > 1 ? "s" : ""}. Mark at least 3 swings, highs and lows, and the lab finds the swing setting that sees them the way you do.`
    : `${n} marks. The closest swing setting is <b>left ${b.left}, right ${b.right}</b>: it finds ${b.found} of your ${n} swings and adds ${b.extra} you didn't mark.
       <span class="row" style="margin-top:8px"><button class="btn small primary" id="lab-marks-apply">Use left ${b.left}, right ${b.right}</button></span>`;
  const ap = $("lab-marks-apply");
  if (ap) ap.onclick = () => { $("lab-left").value = b.left; $("lab-right").value = b.right; swingsChanged(); };
}
$("lab-marks-clear").onclick = () => { L.marks = []; refreshMarkers(); renderMarks(); };

/* ---------- levels ---------- */
function addLevel(price) {
  const id = ++L.levelSeq;
  const line = L.series.createPriceLine({ price, color: css("--accent2"), lineWidth: 2, lineStyle: 0, axisLabelVisible: true, title: "L" + id });
  const lv = { id, price, line }; L.levels.push(lv);
  return lv;
}
function removeLevel(id) {
  const lv = L.levels.find(l => l.id === id); if (!lv) return;
  L.series.removePriceLine(lv.line); L.levels = L.levels.filter(l => l !== lv);
  if (L.setup) L.setup.items = L.setup.items.filter(it => it.level !== id);
  renderLevels(); levelsChanged();
}
function renderLevels() {
  $("lab-levels").hidden = !L.levels.length;
  $("lab-levels-list").innerHTML = L.levels.map(l => `<div class="lv"><i></i><span>L${l.id}</span><input type="number" step="any" data-lv="${l.id}" value="${+l.price.toPrecision(6)}" style="width:120px"><span class="spacer"></span>${L.setup ? `<button class="btn small" data-use="${l.id}">Use in rules</button>` : ""}<button class="x" data-del="${l.id}" title="Delete">×</button></div>`).join("")
    + `<p class="hint">A drawn level is a fixed price, so a rule that uses it only makes sense on this market.</p>`;
  $("lab-levels-list").querySelectorAll("[data-lv]").forEach(inp => inp.onchange = () => {
    const lv = L.levels.find(l => l.id === +inp.dataset.lv), p = parseFloat(inp.value);
    if (lv && p > 0) { lv.price = p; lv.line.applyOptions({ price: p }); levelsChanged(); }
  });
  $("lab-levels-list").querySelectorAll("[data-del]").forEach(b => b.onclick = () => removeLevel(+b.dataset.del));
  $("lab-levels-list").querySelectorAll("[data-use]").forEach(b => b.onclick = () => {
    const it = L.setup.items.find(x => x.level === +b.dataset.use); if (it) it.on = true;
    renderRules(); rescan();
  });
}
function levelsChanged() {
  if (!L.setup) return;
  syncLevelItems(); renderRules(); rescan();
}
function syncLevelItems() {
  const s = L.setup;
  for (const lv of L.levels) {
    const p = +lv.price.toPrecision(6);
    const rules = [{ left: "low", op: "<=", right: p }, { left: "high", op: ">=", right: p }];
    const it = s.items.find(x => x.level === lv.id);
    if (it) { it.rules = rules; it.label = `Touches your level L${lv.id} (${priceFmt(p)})`; }
    else s.items.push({ id: "lv" + lv.id, level: lv.id, group: "Your levels", label: `Touches your level L${lv.id} (${priceFmt(p)})`, rules, on: false });
  }
}

/* ---------- turning candles into rules ---------- */
function fingerprint(a, b) {
  const c = L.candles, n = b - a + 1, items = [];
  const name = i => n === 1 ? "The candle" : `Candle ${i + 1}`;
  // weight: how central a rule is to the shape; "Loosen" drops the lowest first.
  let w = 1;
  const add = (group, label, rules, on = true) => items.push({ id: "r" + items.length, group, label, rules: Array.isArray(rules) ? rules : [rules], on, weight: w });
  for (let i = 0; i < n; i++) {
    const k = n - 1 - i, x = c[a + i], q = parts(x), P = name(i), signal = i === n - 1;
    if (q.range <= 0) continue;
    // Older candles contribute their direction by default; the signal candle its whole shape.
    const sh = signal;
    w = signal ? 4 : 2;
    const g = n === 1 ? "Candle shape" : `${P}${i === n - 1 ? " (signal)" : ""}`;
    if (q.body <= 0.1 * q.range) add(g, `${P} has a tiny body (doji)`, { left: ref("body", k), op: "<=", right: scaled(0.15, "range", k) });
    else {
      const up = x.close > x.open;
      add(g, `${P} closes ${up ? "up (green)" : "down (red)"}`, { left: ref("close", k), op: up ? ">" : "<", right: ref("open", k) });
      w = signal ? 3 : 1;
      if (q.body >= 0.65 * q.range) add(g, `${P} has a strong body`, { left: ref("body", k), op: ">=", right: scaled(0.6, "range", k) }, sh);
      else if (q.body <= 0.35 * q.range) add(g, `${P} has a small body`, { left: ref("body", k), op: "<=", right: scaled(0.4, "range", k) }, sh);
    }
    w = signal ? 3 : 1;
    if (q.lower >= 0.4 * q.range) add(g, `${P} has a long lower wick`, { left: ref("lower_wick", k), op: ">=", right: scaled(0.4, "range", k) }, sh);
    else if (q.lower <= 0.08 * q.range) add(g, `${P} has almost no lower wick`, { left: ref("lower_wick", k), op: "<=", right: scaled(0.1, "range", k) }, false);
    if (q.upper >= 0.4 * q.range) add(g, `${P} has a long upper wick`, { left: ref("upper_wick", k), op: ">=", right: scaled(0.4, "range", k) }, sh);
    else if (q.upper <= 0.08 * q.range) add(g, `${P} has almost no upper wick`, { left: ref("upper_wick", k), op: "<=", right: scaled(0.1, "range", k) }, false);
    // Compared with the candle before it: on for the signal candle, off for older ones.
    const pi = a + i - 1; if (pi < 0) continue;
    const pr = c[pi], pq = parts(pr), inside = i > 0, on = signal;
    w = signal ? 2 : 1;
    const vs = inside ? `${P} vs ${i}` : n === 1 ? "vs the candle before" : `${P} vs the candle before`;
    const rg = n === 1 ? "Compared with the candle before" : g;
    if (x.high !== pr.high) add(rg, `${vs}: ${x.high > pr.high ? "higher high" : "lower high"}`, { left: ref("high", k), op: x.high > pr.high ? ">" : "<", right: ref("high", k + 1) }, on);
    if (x.low !== pr.low) add(rg, `${vs}: ${x.low > pr.low ? "higher low" : "lower low"}`, { left: ref("low", k), op: x.low > pr.low ? ">" : "<", right: ref("low", k + 1) }, on);
    if (x.close > pr.high) add(rg, `${vs}: closes above its high`, { left: ref("close", k), op: ">", right: ref("high", k + 1) }, on);
    else if (x.close < pr.low) add(rg, `${vs}: closes below its low`, { left: ref("close", k), op: "<", right: ref("low", k + 1) }, on);
    if (pq.body > 0 && q.body > pq.body && Math.max(x.open, x.close) >= Math.max(pr.open, pr.close) && Math.min(x.open, x.close) <= Math.min(pr.open, pr.close))
      add(rg, `${vs}: body engulfs it`, { left: ref("body", k), op: ">", right: ref("body", k + 1) }, on);
    else if (pq.body > 0 && q.body < 0.5 * pq.body) add(rg, `${vs}: less than half its body`, { left: ref("body", k), op: "<", right: scaled(0.5, "body", k + 1) }, on);
    if (pq.range > 0 && q.range > 1.6 * pq.range) add(rg, `${vs}: range 1.5× bigger`, { left: ref("range", k), op: ">", right: scaled(1.5, "range", k + 1) }, on);
    else if (pq.range > 0 && q.range < 0.55 * pq.range) add(rg, `${vs}: much smaller range`, { left: ref("range", k), op: "<", right: scaled(0.6, "range", k + 1) }, on);
  }
  // Where it happened. Pre-ticked when the selected example had it.
  w = 2;
  const st = L.state, s = b, x = c[s], ctx = "Where it happens";
  const sup = st?.sup[s], res = st?.res[s], sup1 = st?.sup[s - 1], res1 = st?.res[s - 1];
  add(ctx, "Near swing support", [{ left: "low", op: "<=", right: "1.005 * sw.support" }, { left: "close", op: ">", right: "sw.support" }], sup != null && x.low <= 1.005 * sup && x.close > sup);
  add(ctx, "Near swing resistance", [{ left: "high", op: ">=", right: "0.995 * sw.resistance" }, { left: "close", op: "<", right: "sw.resistance" }], res != null && x.high >= 0.995 * res && x.close < res);
  add(ctx, "Breaks above swing resistance", { left: "close", op: "crosses_above", right: "sw.resistance" }, s > 0 && res != null && res1 != null && x.close > res && c[s - 1].close <= res1);
  add(ctx, "Breaks below swing support", { left: "close", op: "crosses_below", right: "sw.support" }, s > 0 && sup != null && sup1 != null && x.close < sup && c[s - 1].close >= sup1);
  add(ctx, "Uptrend: higher highs and higher lows", { left: "sw.structure", op: "==", right: 1 }, st?.structure[s] === 1);
  add(ctx, "Downtrend: lower highs and lower lows", { left: "sw.structure", op: "==", right: -1 }, st?.structure[s] === -1);
  const pd = periodLevels(c, 1440);
  add(ctx, "Closes above yesterday's high", { left: "close", op: ">", right: "pd.prev_high" }, pd.hi[s] != null && x.close > pd.hi[s]);
  add(ctx, "Closes below yesterday's low", { left: "close", op: "<", right: "pd.prev_low" }, pd.lo[s] != null && x.close < pd.lo[s]);
  add(ctx, "Dips under yesterday's low", { left: "low", op: "<", right: "pd.prev_low" }, pd.lo[s] != null && x.low < pd.lo[s] && x.close >= pd.lo[s]);
  add(ctx, "Pokes above yesterday's high", { left: "high", op: ">", right: "pd.prev_high" }, pd.hi[s] != null && x.high > pd.hi[s] && x.close <= pd.hi[s]);
  const prior = c.slice(Math.max(0, s - 20), s), avg = prior.reduce((t, y) => t + y.volume, 0) / (prior.length || 1);
  add(ctx, "Volume 1.5× the recent average", { left: "vol.relative", op: ">", right: 1.5 }, prior.length >= 20 && avg > 0 && x.volume > 1.5 * avg);
  for (const p of patternsAt(c, s)) add("Named pattern", `Completes a ${PATTERN_NAME[p].toLowerCase()}`, { left: `pa.${p}`, op: "==", right: 1 }, false);
  return items;
}
function makeSetup(a, b) {
  const c = L.candles, bars = c.slice(Math.max(0, a - 1), b + 1).map((x, j) => ({ o: x.open, h: x.high, l: x.low, c: x.close, ctx: a > 0 && j === 0 }));
  const sel = c.slice(a, b + 1), n = sel.length;
  const lowK = n - 1 - sel.reduce((m, x, j) => x.low < sel[m].low ? j : m, 0);
  const highK = n - 1 - sel.reduce((m, x, j) => x.high > sel[m].high ? j : m, 0);
  const last = c[b];
  L.dir = last.close >= last.open ? "long" : "short";
  L.setup = { n, bars, lowK, highK, items: fingerprint(a, b), named: patternsAt(c, b), when: c[b].ts, symbol: L.symbol, interval: L.interval };
  syncLevelItems();
  L.trades = []; $("lab-bt-out").innerHTML = ""; $("lab-cmp").innerHTML = "";
  renderSetup(); renderRules(); renderLevels(); rescan();
}
function miniSvg(bars) {
  const W = 220, H = 100, lo = Math.min(...bars.map(b => b.l)), hi = Math.max(...bars.map(b => b.h));
  const y = v => 8 + (hi - v) / (hi - lo || 1) * (H - 26), step = Math.min(40, (W - 30) / Math.max(1, bars.length - 1)), x0 = (W - step * (bars.length - 1)) / 2;
  let num = 0;
  return `<svg viewBox="0 0 ${W} ${H}" aria-hidden="true">` + bars.map((b, i) => {
    const x = x0 + i * step, col = b.c >= b.o ? "var(--up)" : "var(--down)", top = y(Math.max(b.o, b.c)), bh = Math.max(1.5, Math.abs(y(b.o) - y(b.c)));
    const label = b.ctx ? "" : `<text x="${x}" y="${H - 4}" font-size="10" text-anchor="middle" fill="currentColor" opacity=".6">${++num}</text>`;
    return `<g opacity="${b.ctx ? .3 : 1}"><line x1="${x}" y1="${y(b.h)}" x2="${x}" y2="${y(b.l)}" stroke="${col}" stroke-width="2"/><rect x="${x - 8}" y="${top}" width="16" height="${bh}" rx="2" fill="${col}"/></g>${label}`;
  }).join("") + `</svg>`;
}
function describe(s) {
  const b = s.bars[s.bars.length - 1], r = b.h - b.l, pc = v => r > 0 ? Math.round(v / r * 100) + "%" : "–";
  const body = Math.abs(b.c - b.o), up = b.h - Math.max(b.o, b.c), dn = Math.min(b.o, b.c) - b.l;
  return `${s.n} candle${s.n > 1 ? "s" : ""} on ${esc(s.symbol)} ${esc(s.interval)}, ending ${dt(s.when)} UTC. The signal candle's body is ${pc(body)} of its range, with wicks of ${pc(up)} above and ${pc(dn)} below.`;
}
function renderSetup() {
  const s = L.setup;
  ["lab-s2", "lab-s3", "lab-s4"].forEach(id => $(id).classList.toggle("dim", !s));
  $("lab-clear").hidden = !s;
  if (!s) { $("lab-pick").innerHTML = `<div class="empty-state">Choose <b>Select candles</b> and drag across <b>1 to 5 candles</b> on the chart: a rejection wick, an engulfing, a breakout candle, anything you'd trade.</div>`; return; }
  $("lab-pick").innerHTML = `<div class="pick"><div style="color:var(--ink2)">${miniSvg(s.bars)}</div><div><p>${describe(s)}</p>
    ${s.named.length ? `<div class="named">${s.named.map(p => `<span>${esc(PATTERN_NAME[p])}</span>`).join("")}</div>` : ""}</div></div>`;
  $("lab-dir").querySelectorAll("button").forEach(b => b.classList.toggle("on", b.dataset.dir === L.dir));
}
$("lab-clear").onclick = () => { L.setup = null; L.sel = null; L.matches = []; L.trades = []; drawSel(); renderSetup(); renderLevels(); refreshMarkers(); $("lab-rules").innerHTML = ""; statsEmpty(); };
function ruleText(r) { return `${r.left} ${r.op === "crosses_above" ? "crosses above" : r.op === "crosses_below" ? "crosses below" : r.op} ${r.right}`; }
function renderRules() {
  const s = L.setup; if (!s) return;
  let html = "", g = null;
  for (const it of s.items) {
    if (it.group !== g) { g = it.group; html += `<div class="grp-t">${esc(g)}</div>`; }
    html += `<label title="${esc(it.rules.map(ruleText).join("  and  "))}"><input type="checkbox" data-it="${it.id}" ${it.on ? "checked" : ""}><span>${esc(it.label)}</span>${it.custom ? `<button class="x" data-rm="${it.id}" title="Remove">×</button>` : `<code>${esc(ruleText(it.rules[0]))}</code>`}</label>`;
  }
  $("lab-rules").innerHTML = html;
  $("lab-rules").querySelectorAll("[data-it]").forEach(cb => cb.onchange = () => { s.items.find(x => x.id === cb.dataset.it).on = cb.checked; rescan(); });
  $("lab-rules").querySelectorAll("[data-rm]").forEach(b => b.onclick = e => { e.preventDefault(); s.items = s.items.filter(x => x.id !== b.dataset.rm); renderRules(); rescan(); });
}
$("lab-dir").querySelectorAll("button").forEach(b => b.onclick = () => { L.dir = b.dataset.dir; renderSetup(); L.trades = []; rescan(); });
function addCustom() {
  const t = $("lab-add").value.trim(); if (!t || !L.setup) return;
  const m = /^(.+?)\s*(>=|<=|==|!=|>|<|crosses above|crosses below)\s*(.+)$/i.exec(t);
  if (!m) { $("lab-add-err").textContent = "Write it as: value, comparison, value. For example close > 1.01 * high[1]"; return; }
  const num = v => /^-?\d+(\.\d+)?$/.test(v.trim()) ? parseFloat(v) : v.trim();
  const op = m[2].toLowerCase().replace(" ", "_");
  L.setup.items.push({ id: "c" + Date.now(), group: "Your rules", label: t, custom: true, rules: [{ left: m[1].trim(), op, right: num(m[3]) }], on: true });
  $("lab-add").value = ""; $("lab-add-err").textContent = "";
  renderRules(); rescan();
}
$("lab-add-btn").onclick = addCustom;
$("lab-add").addEventListener("keydown", e => { if (e.key === "Enter") addCustom(); });

/* ---------- the strategy the rules describe ---------- */
function activeRules() { return L.setup ? L.setup.items.filter(i => i.on).flatMap(i => i.rules) : []; }
function strategy() {
  const s = L.setup, rules = activeRules(), long = L.dir === "long";
  const text = JSON.stringify(rules) + L.stop + L.target;
  const ind = {};
  if (/sw\./.test(text) || L.target === "swing") ind.sw = { type: "swings", left: +$("lab-left").value || 3, right: +$("lab-right").value || 3 };
  if (/pd\./.test(text)) ind.pd = { type: "period", minutes: 1440 };
  if (/vol\./.test(text)) ind.vol = { type: "volume_avg", period: 20 };
  if (/pa\./.test(text)) ind.pa = { type: "patterns" };
  if (L.stop === "atr") ind.atr = { type: "atr", period: 14 };
  const k = long ? s.lowK : s.highK;
  const stop = L.stop === "atr" ? { atr: 1.5, indicator: "atr" }
    : L.stop === "candle" ? (long ? { below: "low - 0.1 * range" } : { above: "high + 0.1 * range" })
    : long ? { below: `${ref("low", k)} - ${scaled(0.1, "range", k)}` } : { above: `${ref("high", k)} + ${scaled(0.1, "range", k)}` };
  const target = L.target === "swing" ? (long ? { above: "sw.resistance" } : { below: "sw.support" }) : { risk_multiple: +L.target };
  const names = s.named.map(p => PATTERN_NAME[p]).join(", ");
  return {
    name: `Lab setup: ${s.n}-candle ${long ? "long" : "short"}${names ? " (" + names + ")" : ""}`,
    description: `Found in the Chart Lab on ${s.symbol} ${s.interval}. ${s.items.filter(i => i.on).map(i => i.label).join("; ")}.`,
    market: { symbol: L.symbol === "CSV" ? s.symbol : L.symbol, interval: L.interval },
    indicators: ind,
    entry: { [L.dir]: rules.length === 1 ? rules[0] : { all: rules } },
    exit: { stop_loss: stop, take_profit: target, max_bars: Math.max(1, parseInt($("lab-maxbars").value) || 24) },
    sizing: { type: "risk_percent", value: 1 },
  };
}
function syncJson() {
  if (L.setup && activeRules().length) $("lab-json").textContent = JSON.stringify(strategy(), null, 2);
  stopHint();
}
/** Warns when the stop is so close that trading costs are a large part of the risk. */
function stopHint() {
  const s = L.setup, el = $("lab-stop-hint"); if (!s || L.stop === "atr") { el.textContent = ""; return; }
  const bars = s.bars.filter(b => !b.ctx), sig = bars[bars.length - 1], long = L.dir === "long";
  const k = long ? s.lowK : s.highK, b = L.stop === "candle" ? sig : bars[bars.length - 1 - k], range = b.h - b.l;
  const stop = long ? b.l - 0.1 * range : b.h + 0.1 * range, dist = Math.abs(sig.c - stop) / sig.c * 100;
  const cost = 0.24; // 0.1% fee and 0.02% slippage, in and out
  el.innerHTML = dist <= 0 ? "" : dist < 5 * cost
    ? `<span style="color:var(--warn)">This stop was ${fmt(dist, 2)}% away on your example, so costs of about ${cost}% a round trip take ~${fmt(cost / dist, 1)}R from every trade. Try <b>1.5 ATR</b> or a higher timeframe.</span>`
    : `Stop ${fmt(dist, 2)}% from the entry on your example.`;
}

/* ---------- scanning ---------- */
function rescan() {
  clearTimeout(L.scanTimer);
  L.scanTimer = setTimeout(doScan, 180);
}
async function doScan() {
  if (!L.setup || !L.candles.length) return;
  syncJson();
  if (!activeRules().length) { L.matches = []; L.cur = -1; refreshMarkers(); statsEmpty("Switch on at least one rule."); return; }
  const seq = ++L.scanSeq;
  $("lab-count-sub").innerHTML = `<span class="spinner" style="border-color:var(--line2);border-top-color:var(--accent)"></span>`;
  const r = await api("/api/scan", { strategy: strategy(), ...dataReq() });
  if (seq !== L.scanSeq) return;
  if (!r.ok) { L.matches = []; refreshMarkers(); statsEmpty(r.errors.join(" ")); return; }
  L.matches = r[L.dir].map(ts => L.byTs.get(ts)).filter(i => i != null);
  L.cur = -1; L.trades = [];
  refreshMarkers(); stats();
}
function step(d) {
  const n = L.matches.length; if (!n) return;
  L.cur = L.cur < 0 ? (d > 0 ? 0 : n - 1) : (L.cur + d + n) % n;
  const i = L.matches[L.cur];
  L.chart.timeScale().setVisibleLogicalRange({ from: i - 70, to: i + 40 });
  refreshMarkers();
}
$("lab-prev").onclick = () => step(-1);
$("lab-next").onclick = () => step(1);

/* ---------- what happened next ---------- */
function forward(idx, H, s) {
  const c = L.candles, paths = [], fin = [], mfe = [], mae = [];
  for (const t of idx) {
    if (t + H >= c.length) continue;
    const e = c[t + 1].open, p = [0];
    let best = -Infinity, worst = Infinity;
    for (let h = 1; h <= H; h++) {
      const b = c[t + h];
      p.push(s * (b.close / e - 1));
      const fav = s > 0 ? b.high / e - 1 : 1 - b.low / e, adv = s > 0 ? b.low / e - 1 : 1 - b.high / e;
      best = Math.max(best, fav); worst = Math.min(worst, adv);
    }
    paths.push(p); fin.push(p[H]); mfe.push(best); mae.push(worst);
  }
  return { paths, fin, mfe, mae };
}
const mean = a => a.reduce((t, x) => t + x, 0) / (a.length || 1);
const quant = (a, q) => { if (!a.length) return 0; const s = [...a].sort((x, y) => x - y); return s[Math.min(s.length - 1, Math.floor(q * (s.length - 1)))]; };
function statsEmpty(msg) {
  $("lab-count").textContent = "0"; $("lab-count-sub").textContent = msg || "matches";
  $("lab-stats").innerHTML = ""; $("lab-path").innerHTML = "";
}
/** Switches off the least central rules until there are enough matches to learn from. */
async function loosen(btn) {
  const target = Math.max(10, Math.round(L.candles.length / 400));
  btn.disabled = true; btn.textContent = "Loosening…";
  try {
    while (L.matches.length < target) {
      const on = L.setup.items.filter(i => i.on);
      if (on.length <= 1) break;
      const drop = on.reduce((m, i) => (i.weight ?? 1) < (m.weight ?? 1) ? i : m, on[on.length - 1]);
      drop.on = false;
      await doScan();
    }
  } finally { renderRules(); stats(); }
}
function stats() {
  const H = +$("lab-h").value, s = L.dir === "long" ? 1 : -1, n = L.matches.length;
  $("lab-h-v").textContent = `${H} candle${H > 1 ? "s" : ""}`;
  $("lab-count").textContent = n.toLocaleString();
  const perMonth = n / Math.max(1, (L.candles[L.candles.length - 1].ts - L.candles[0].ts) / (30 * 86400000));
  const few = n < Math.max(10, Math.round(L.candles.length / 400));
  $("lab-count-sub").innerHTML = (n ? `matches · about ${perMonth >= 10 ? Math.round(perMonth) : fmt(perMonth, 1)} a month` : "matches")
    + (few ? ` <button class="btn small" id="lab-loosen" title="Switch off the least important rules until there are enough matches">Too few? Loosen</button>` : "");
  if (few) $("lab-loosen").onclick = e => loosen(e.currentTarget);
  if (!n) { $("lab-stats").innerHTML = ""; $("lab-path").innerHTML = ""; return; }
  const f = forward(L.matches, H, s);
  const all = forward(L.candles.map((_, i) => i).filter((_, i) => i % Math.max(1, Math.floor(L.candles.length / 3000)) === 0), H, s);
  const win = f.fin.filter(x => x > 0).length / (f.fin.length || 1), base = all.fin.filter(x => x > 0).length / (all.fin.length || 1);
  const cell = (v, l, c = "") => `<div><b class="${c}">${v}</b><span>${l}</span></div>`;
  $("lab-stats").innerHTML = f.fin.length ? [
    cell(fmt(win * 100, 0) + "%", `went your way (any candle: ${fmt(base * 100, 0)}%)`, win > base + 0.02 ? "up" : win < base - 0.02 ? "down" : ""),
    cell(signPct(mean(f.fin)), "average move", cls(mean(f.fin))),
    cell(signPct(quant(f.fin, 0.5)), "median move", cls(quant(f.fin, 0.5))),
    cell(signPct(mean(f.mfe)), "average run in your favour", "up"),
    cell(signPct(mean(f.mae)), "average run against you", "down"),
    cell(`${signPct(Math.max(...f.fin))}<br>${signPct(Math.min(...f.fin))}`, "best / worst"),
  ].join("") : `<p class="hint" style="grid-column:1/-1">Every match is too close to the end of the chart to measure ${H} candles ahead.</p>`;
  $("lab-path").innerHTML = f.paths.length ? pathSvg(f.paths, all.paths, H) : "";
}
function pathSvg(paths, basePaths, H) {
  const W = 360, Hh = 150, avg = [], p25 = [], p75 = [], base = [];
  for (let h = 0; h <= H; h++) {
    const col = paths.map(p => p[h]);
    avg.push(mean(col)); p25.push(quant(col, 0.25)); p75.push(quant(col, 0.75));
    base.push(mean(basePaths.map(p => p[h])));
  }
  const lo = Math.min(0, ...p25, ...base), hi = Math.max(0, ...p75, ...base);
  const X = h => 10 + h / H * (W - 60), Y = v => 10 + (hi - v) / (hi - lo || 1) * (Hh - 28);
  const pts = a => a.map((v, h) => `${X(h).toFixed(1)},${Y(v).toFixed(1)}`).join(" ");
  const band = pts(p75) + " " + p25.map((v, h) => [h, v]).reverse().map(([h, v]) => `${X(h).toFixed(1)},${Y(v).toFixed(1)}`).join(" ");
  const end = avg[H], col = end >= 0 ? "var(--up)" : "var(--down)";
  return `<svg class="path-svg" viewBox="0 0 ${W} ${Hh}" role="img" aria-label="Average path after a match">
    <line x1="10" x2="${W - 50}" y1="${Y(0)}" y2="${Y(0)}" stroke="currentColor" stroke-opacity=".25" stroke-dasharray="3 3"/>
    <polygon points="${band}" fill="var(--accent)" fill-opacity=".13"/>
    <polyline points="${pts(base)}" fill="none" stroke="var(--faint)" stroke-width="1.3" stroke-dasharray="4 3"/>
    <polyline points="${pts(avg)}" fill="none" stroke="var(--accent)" stroke-width="2.4" stroke-linejoin="round"/>
    <circle cx="${X(H)}" cy="${Y(end)}" r="3.5" fill="${col}"/>
    <text x="${X(H) + 6}" y="${Y(end) + 4}" font-size="11" font-weight="700" fill="${col}">${signPct(end)}</text>
    <text x="10" y="${Hh - 6}" font-size="10" fill="currentColor" opacity=".55">match</text>
    <text x="${W - 50}" y="${Hh - 6}" font-size="10" fill="currentColor" opacity=".55" text-anchor="end">+${H} candles</text>
  </svg><p class="hint" style="margin-top:6px"><b style="color:var(--accent)">━</b> average after a match, shaded: middle half of outcomes. <b style="color:var(--faint)">┅</b> any candle.</p>`;
}
$("lab-h").oninput = () => { if (L.setup && L.matches.length) stats(); else $("lab-h-v").textContent = `${$("lab-h").value} candles`; };

/* ---------- backtests ---------- */
function kpiCells(m) {
  const c = (v, l, k = "") => `<div><b class="${k}">${v}</b><span>${l}</span></div>`;
  return c(pct(m.total_return_pct), "return", cls(m.total_return_pct)) + c(m.trades, "trades") + c(fmt(m.win_rate_pct, 0) + "%", "win rate")
    + c(m.avg_r == null ? "–" : rmul(m.avg_r), "average R", m.avg_r == null ? "" : cls(m.avg_r)) + c("−" + fmt(m.max_drawdown_pct, 1) + "%", "max drawdown")
    + c(m.profit_factor == null ? (m.trades ? "∞" : "–") : fmt(m.profit_factor), "profit factor", m.profit_factor != null && m.profit_factor < 1 ? "down" : "");
}
$("lab-bt").onclick = async e => {
  if (!L.setup || !activeRules().length) return;
  const b = e.currentTarget, old = b.textContent; b.disabled = true; b.innerHTML = `<span class="spinner"></span> Testing`;
  try {
    const req = { strategy: strategy(), ...dataReq(), capital: 10000 };
    const r = await api("/api/backtest", req);
    if (!r.ok) { $("lab-bt-out").innerHTML = `<div class="errors" style="margin-top:12px">${r.errors.map(esc).join("<br>")}</div>`; return; }
    const m = r.report.metrics;
    L.trades = r.report.trades; refreshMarkers();
    $("lab-bt-out").innerHTML = `<div class="lab-kpis">${kpiCells(m)}</div>
      <p class="hint">Buy and hold: ${pct(m.buy_hold_return_pct)}. Includes 0.1% fees and slippage; 1% of the account risked per trade. Exits are marked on the chart.</p>
      <div class="row"><button class="btn small" id="lab-full">Full report</button></div>`;
    $("lab-full").onclick = () => { loadStrategy(structuredClone(req.strategy)); LAST = r; LAST_REQ = req; VERIFY = null; renderResults(r); show("results"); };
  } finally { b.disabled = false; b.textContent = old; }
};
async function compare(kind, btn) {
  if (!L.setup || !activeRules().length) return;
  const list = kind === "tf" ? TIMEFRAMES : COINS;
  const old = btn.textContent; btn.disabled = true;
  const head = kind === "tf" ? "Timeframe" : "Market";
  $("lab-cmp").innerHTML = `<div class="tw" style="margin-top:12px"><table class="tf-table"><thead><tr><th>${head}</th><th class="n">Trades</th><th class="n">Win</th><th class="n">Avg R</th><th class="n">Return</th><th class="n">Max DD</th></tr></thead><tbody id="lab-cmp-b"></tbody></table></div>
    <p class="hint">${kind === "tf" ? "Same rules on each timeframe's default period (a month of 5m up to five years of 1d)." : `Same rules and period on each market, at ${esc(L.interval)}.`} A setup that only works on one row is probably luck.</p>`;
  const s = strategy();
  try {
    for (const x of list) {
      btn.innerHTML = `<span class="spinner" style="border-color:var(--line2);border-top-color:var(--accent)"></span> ${x}`;
      const req = kind === "tf" ? { strategy: { ...s, market: { ...s.market, interval: x } }, symbol: L.symbol === "CSV" ? "BTCUSDT" : L.symbol, interval: x }
        : { strategy: s, ...dataReq(), symbol: x, csv: null };
      const r = await api("/api/backtest", req);
      const row = r.ok ? (m => `<td class="n">${m.trades}</td><td class="n">${fmt(m.win_rate_pct, 0)}%</td><td class="n ${m.avg_r == null ? "" : cls(m.avg_r)}">${m.avg_r == null ? "–" : rmul(m.avg_r)}</td><td class="n ${cls(m.total_return_pct)}">${pct(m.total_return_pct)}</td><td class="n">−${fmt(m.max_drawdown_pct, 1)}%</td>`)(r.report.metrics)
        : `<td colspan="5" class="hint">${esc(r.errors[0])}</td>`;
      const cur = kind === "tf" ? x === L.interval : x === L.symbol;
      $("lab-cmp-b").insertAdjacentHTML("beforeend", `<tr${cur ? ` style="background:var(--accent-soft)"` : ""}><td><b>${esc(x)}</b></td>${row}</tr>`);
    }
  } finally { btn.disabled = false; btn.textContent = old; }
}
$("lab-tfs").onclick = e => compare("tf", e.currentTarget);
$("lab-coins").onclick = e => compare("coin", e.currentTarget);
const pick = (id, key) => $(id).querySelectorAll("button").forEach(b => b.onclick = () => {
  L[key] = b.dataset.v; $(id).querySelectorAll("button").forEach(x => x.classList.toggle("on", x === b)); syncJson(); L.trades = []; refreshMarkers();
});
pick("lab-stop", "stop"); pick("lab-target", "target");
$("lab-maxbars").onchange = syncJson;
$("lab-open").onclick = () => { if (L.setup && activeRules().length) { loadStrategy(strategy()); show("build"); } };
$("lab-copy").onclick = e => { if (!L.setup) return; navigator.clipboard.writeText(JSON.stringify(strategy(), null, 2)).then(() => { e.target.textContent = "Copied"; setTimeout(() => e.target.textContent = "Copy JSON", 1500); }); };

/* ---------- saved setups (this browser only) ---------- */
const SAVE_KEY = "candlerail-lab-setups";
const loadSaved = () => { try { return JSON.parse(localStorage.getItem(SAVE_KEY) || "[]"); } catch (e) { return []; } };
const storeSaved = v => { try { localStorage.setItem(SAVE_KEY, JSON.stringify(v)); } catch (e) {} };
function renderSaved() {
  const v = loadSaved();
  $("lab-saved-box").hidden = !v.length;
  $("lab-saved").innerHTML = v.map((x, i) => `<div><span title="${esc(x.name)}">${esc(x.name)}</span><button class="btn small" data-load="${i}">Load</button><button class="x" data-drop="${i}" title="Delete">×</button></div>`).join("");
  $("lab-saved").querySelectorAll("[data-load]").forEach(b => b.onclick = () => {
    const x = loadSaved()[+b.dataset.load]; if (!x) return;
    L.setup = x.setup; L.dir = x.dir; L.stop = x.stop; L.target = x.target; $("lab-maxbars").value = x.maxBars;
    L.setup.items = L.setup.items.filter(it => !it.level);
    [["lab-stop", L.stop], ["lab-target", L.target]].forEach(([id, v]) => $(id).querySelectorAll("button").forEach(bb => bb.classList.toggle("on", bb.dataset.v === v)));
    L.sel = null; drawSel(); syncLevelItems(); renderSetup(); renderRules(); rescan();
  });
  $("lab-saved").querySelectorAll("[data-drop]").forEach(b => b.onclick = () => { const v2 = loadSaved(); v2.splice(+b.dataset.drop, 1); storeSaved(v2); renderSaved(); });
}
$("lab-save").onclick = e => {
  if (!L.setup) return;
  const s = L.setup, names = s.named.map(p => PATTERN_NAME[p]).join(", ");
  const name = `${s.n}-candle ${L.dir}${names ? " · " + names : ""} · ${s.symbol} ${s.interval} · ${s.items.filter(i => i.on).length} rules`;
  const v = loadSaved(); v.unshift({ name, setup: s, dir: L.dir, stop: L.stop, target: L.target, maxBars: $("lab-maxbars").value }); storeSaved(v.slice(0, 30));
  renderSaved(); e.target.textContent = "Saved"; setTimeout(() => e.target.textContent = "Save setup", 1500);
};

/* ---------- pattern overlay ---------- */
async function scanPattern() {
  const p = $("lab-pattern").value;
  if (!p || !L.candles.length) { L.patternHits = []; refreshMarkers(); foot(); return; }
  const r = await api("/api/scan", { strategy: { name: "pattern", indicators: { pa: { type: "patterns" } }, entry: { long: { left: `pa.${p}`, op: "==", right: 1 } }, exit: { max_bars: 1 } }, ...dataReq() });
  L.patternHits = r.ok ? r.long.map(ts => L.byTs.get(ts)).filter(i => i != null) : [];
  refreshMarkers();
  foot(`${L.patternHits.length} ${esc(PATTERN_NAME[p].toLowerCase())}${L.patternHits.length === 1 ? "" : "s"} marked`);
}

/* ---------- controls ---------- */
function swingsChanged() { if (!L.candles.length) return; drawSwings(); refreshMarkers(); renderMarks(); if (L.setup && /sw\./.test(JSON.stringify(activeRules()) + L.target)) rescan(); }
$("lab-zz").onchange = swingsChanged; $("lab-left").onchange = swingsChanged; $("lab-right").onchange = swingsChanged;
$("lab-period").onchange = () => { drawPeriod(); foot(); };
$("lab-vol").onchange = drawVolume;
$("lab-pattern").onchange = scanPattern;
$("lab-load").onclick = loadChart;
$("lab-symbol").addEventListener("keydown", e => { if (e.key === "Enter") loadChart(); });
$("lab-csv").onchange = async e => {
  const f = e.target.files[0]; if (!f) return;
  L.csv = await f.text(); L.csvName = f.name; e.target.value = ""; loadChart();
};
$("lab-help-btn").onclick = () => { $("lab-help").hidden = !$("lab-help").hidden; };

(async function init() {
  while (!CAT) await new Promise(r => setTimeout(r, 50));
  renderMarketControls(); renderSaved(); hint();
  if (activeTab === "lab") loadChart();
})();
})();
