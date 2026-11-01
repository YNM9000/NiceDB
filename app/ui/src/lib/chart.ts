import { m } from '../paraglide/messages';

export type ChartKind = 'bar' | 'hbar' | 'line' | 'area' | 'pie' | 'scatter';
export type Agg = 'none' | 'sum' | 'count' | 'avg' | 'min' | 'max';
export type ChartSort = 'none' | 'x' | 'desc' | 'asc';
export type ColKind = 'number' | 'date' | 'text';

export interface ChartSettings {
  kind: ChartKind;
  x: string;
  ys: string[];
  agg: Agg;
  sort: ChartSort;
  top: number;
  stacked?: boolean;
  log?: boolean;
}

export const canStack = (kind: ChartKind) => kind === 'bar' || kind === 'hbar';
export const canLog = (kind: ChartKind) => kind !== 'pie';

export interface ChartColumn {
  name: string;
  type_name?: string | null;
}

type Cell = string | null;

export const SERIES = 6;
export const TOPS = [0, 5, 10, 20, 50] as const;
const DEFAULT_TOP = 10;
export const MAX_POINTS = 5000;
const MAX_SLICES = 40;

const NUMERIC_TYPE = /^(u?(tiny|small|medium|big|huge)?int(eger)?\d*|u?int\d+|(big|small)?serial\d*|numeric|decimal|dec|number|real|double|float\d*|money|smallmoney|fixed)\b/i;
const DATE_TYPE = /^(date|datetime\d*|smalldatetime|datetimeoffset|timestamp(tz)?)\b/i;
const NUMBER = /^[-+]?(\d+\.?\d*|\.\d+)(e[-+]?\d+)?$/i;
const ISO = /^(\d{4})-(\d{2})-(\d{2})(?:[T ](\d{2}):(\d{2})(?::(\d{2})(?:\.(\d+))?)?)?\s*(Z|[+-]\d{2}(?::?\d{2})?)?$/;

export function parseNumber(v: Cell): number | null {
  if (v === null) return null;
  const t = v.trim();
  if (!NUMBER.test(t)) return null;
  const n = Number(t);
  return Number.isFinite(n) ? n : null;
}

export function parseDate(v: Cell): number | null {
  if (v === null) return null;
  const m = ISO.exec(v.trim());
  if (!m) return null;
  const [, y, mo, d, h = '0', mi = '0', s = '0', frac = '0', zone] = m;
  const at = new Date(Date.UTC(2000, 0, 1, Number(h), Number(mi), Number(s), Number(`0.${frac}`) * 1000));
  at.setUTCFullYear(Number(y), Number(mo) - 1, Number(d));
  if (at.getUTCMonth() !== Number(mo) - 1 || at.getUTCDate() !== Number(d)) return null;
  let ms = at.getTime();
  if (zone && zone !== 'Z') {
    const sign = zone.startsWith('-') ? -1 : 1;
    const digits = zone.slice(1).replace(':', '');
    ms -= sign * (Number(digits.slice(0, 2)) * 60 + Number(digits.slice(2) || 0)) * 60_000;
  }
  return Number.isNaN(ms) ? null : ms;
}

export function columnKind(type: string | null | undefined, values: readonly Cell[]): ColKind {
  type ??= '';
  if (NUMERIC_TYPE.test(type)) return 'number';
  if (DATE_TYPE.test(type)) return 'date';
  const textual = /char|text|string|clob/i.test(type);
  if (type && !textual && !/^(unknown|any|\?)$/i.test(type)) return 'text';
  let seen = 0;
  let nums = 0;
  let dates = 0;
  for (const v of values) {
    if (v === null) continue;
    if (seen === 50) break;
    seen++;
    if (parseNumber(v) !== null) nums++;
    else if (parseDate(v) !== null) dates++;
  }
  if (!seen) return 'text';
  if (dates === seen) return 'date';
  return !textual && nums === seen ? 'number' : 'text';
}

const isKey = (name: string) => /^(id|.*_id|.*_pk)$/i.test(name);

export function defaults(columns: readonly ChartColumn[], kinds: readonly ColKind[], distinctX: (c: number) => boolean): ChartSettings {
  const at = (k: ColKind) => kinds.findIndex((kind, i) => kind === k && !isKey(columns[i]?.name ?? ''));
  const date = at('date');
  const text = at('text');
  const x = date >= 0 ? date : text >= 0 ? text : 0;
  const ys = columns.flatMap((c, i) => (i !== x && kinds[i] === 'number' && !isKey(c.name) ? [c.name] : []));
  const xKind = kinds[x] ?? 'text';
  const unique = distinctX(x);
  return {
    kind: xKind === 'date' ? 'line' : 'bar',
    x: columns[x]?.name ?? '',
    ys: ys.slice(0, 1),
    agg: ys.length === 0 ? 'count' : unique ? 'none' : 'sum',
    sort: xKind === 'text' && !unique ? 'desc' : xKind === 'text' ? 'none' : 'x',
    top: xKind === 'text' && !unique ? DEFAULT_TOP : 0,
  };
}

export function topOf(n: number): number {
  return n <= 0 ? 0 : (TOPS.find((t) => t >= n) ?? TOPS[TOPS.length - 1] ?? 0);
}

export function fit(saved: ChartSettings | undefined, columns: readonly ChartColumn[], kinds: readonly ColKind[], fresh: () => ChartSettings): ChartSettings {
  if (!saved) return fresh();
  const names = columns.map((c) => c.name);
  if (!names.includes(saved.x)) return { ...fresh(), kind: saved.kind };
  const ys = saved.ys.filter((y) => kinds[names.indexOf(y)] === 'number');
  return { ...saved, ys, top: topOf(saved.top) };
}

export interface Category {
  label: string;
  at: number | null;
  other?: boolean;
}

export interface Series {
  name: string;
  values: (number | null)[];
}

export interface Prepared {
  xKind: ColKind;
  cats: Category[];
  series: Series[];
  skipped: number;
  dropped: number;
}

export interface PrepareInput {
  xs: readonly Cell[];
  xKind: ColKind;
  ys: { name: string; values: readonly Cell[] }[];
  agg: Agg;
  sort: ChartSort;
  top: number;
  kind: ChartKind;
  otherLabel: string;
  locale: string;
}

interface Acc {
  sum: number;
  n: number;
  min: number;
  max: number;
}

const blank = (): Acc => ({ sum: 0, n: 0, min: Infinity, max: -Infinity });

function add(a: Acc, v: number) {
  a.sum += v;
  a.n++;
  if (v < a.min) a.min = v;
  if (v > a.max) a.max = v;
}

function merge(into: Acc, a: Acc) {
  into.sum += a.sum;
  into.n += a.n;
  into.min = Math.min(into.min, a.min);
  into.max = Math.max(into.max, a.max);
}

function valueOf(a: Acc, agg: Agg): number | null {
  if (agg === 'count') return a.n;
  if (!a.n) return null;
  if (agg === 'avg') return a.sum / a.n;
  if (agg === 'min') return a.min;
  if (agg === 'max') return a.max;
  return a.sum;
}

export type DateUnit = 'year' | 'month' | 'day' | 'minute' | 'second';

export function dateUnit(times: Iterable<number>): DateUnit {
  let unit: DateUnit = 'year';
  const rank: DateUnit[] = ['year', 'month', 'day', 'minute', 'second'];
  for (const t of times) {
    const d = new Date(t);
    const need: DateUnit = d.getUTCSeconds() || d.getUTCMilliseconds() ? 'second' : d.getUTCHours() || d.getUTCMinutes() ? 'minute' : d.getUTCDate() !== 1 ? 'day' : d.getUTCMonth() ? 'month' : 'year';
    if (rank.indexOf(need) > rank.indexOf(unit)) unit = need;
    if (unit === 'second') break;
  }
  return unit === 'year' ? 'month' : unit;
}

const continuous = (kind: ChartKind, xKind: ColKind) => xKind !== 'text' && (kind === 'line' || kind === 'area' || kind === 'scatter');

export function prepare(p: PrepareInput): Prepared {
  const parse = p.xKind === 'date' ? parseDate : p.xKind === 'number' ? parseNumber : (v: Cell) => (v === null ? null : 0);
  const count = p.agg === 'count';
  const ys = p.ys.length ? p.ys : count ? [{ name: '', values: p.xs }] : [];
  const keys: string[] = [];
  const ats: (number | null)[] = [];
  const accs: Acc[][] = [];
  const index = new Map<string, number>();
  let skipped = 0;
  const grouped = p.agg !== 'none';
  for (let r = 0; r < p.xs.length; r++) {
    const raw = p.xs[r] ?? null;
    const at = parse(raw);
    if (raw === null || at === null) {
      skipped++;
      continue;
    }
    const key = p.xKind === 'text' ? raw : String(at);
    let c = grouped ? index.get(key) : undefined;
    if (c === undefined) {
      c = keys.length;
      keys.push(raw);
      ats.push(p.xKind === 'text' ? null : at);
      accs.push(ys.map(blank));
      if (grouped) index.set(key, c);
    }
    const row = accs[c];
    if (!row) continue;
    for (let s = 0; s < ys.length; s++) {
      const v = ys[s]?.values[r] ?? null;
      const acc = row[s];
      if (!acc) continue;
      if (count) {
        if (v !== null) add(acc, 1);
      } else {
        const n = parseNumber(v);
        if (n !== null) add(acc, n);
      }
    }
  }

  let order = keys.map((_, i) => i);
  const first = (i: number) => valueOf(accs[i]?.[0] ?? blank(), p.agg);
  const byValue = (dir: 1 | -1) => (a: number, b: number) => {
    const va = first(a);
    const vb = first(b);
    if (va === null || vb === null) return va === null ? (vb === null ? a - b : 1) : -1;
    return (va - vb) * dir || a - b;
  };
  const collator = new Intl.Collator(undefined, { numeric: true, sensitivity: 'base' });
  const byX = (a: number, b: number) => (p.xKind === 'text' ? collator.compare(keys[a] ?? '', keys[b] ?? '') : (ats[a] ?? 0) - (ats[b] ?? 0)) || a - b;

  const pie = p.kind === 'pie';
  if (pie) order = order.filter((i) => (first(i) ?? 0) > 0);
  const cont = continuous(p.kind, p.xKind);
  const top = cont ? 0 : pie ? Math.min(p.top || MAX_SLICES, MAX_SLICES) : p.top;
  let rest: number[] = [];
  if (top > 0 && order.length > top) {
    const ranked = [...order].sort(byValue(-1));
    const kept = new Set(ranked.slice(0, top));
    rest = order.filter((i) => !kept.has(i));
    order = order.filter((i) => kept.has(i));
  }
  if (cont || p.sort === 'x') order.sort(byX);
  else if (p.sort === 'desc') order.sort(byValue(-1));
  else if (p.sort === 'asc') order.sort(byValue(1));

  let dropped = 0;
  if (order.length > MAX_POINTS) {
    dropped = order.length - MAX_POINTS;
    order = order.slice(0, MAX_POINTS);
  }

  const label = dateLabel(p.locale);
  const numLabel = valueFormat(p.locale);
  const unit = p.xKind === 'date' ? dateUnit(order.map((i) => ats[i] ?? 0)) : 'day';
  const cats: Category[] = order.map((i) => ({ label: p.xKind === 'date' ? label(ats[i] ?? 0, unit) : p.xKind === 'number' ? numLabel(ats[i] ?? null) : (keys[i] ?? ''), at: ats[i] ?? null }));
  const series: Series[] = ys.map((y, s) => ({ name: y.name, values: order.map((i) => valueOf(accs[i]?.[s] ?? blank(), p.agg)) }));
  if (rest.length) {
    cats.push({ label: p.otherLabel, at: null, other: true });
    ys.forEach((_, s) => {
      const acc = blank();
      for (const i of rest) merge(acc, accs[i]?.[s] ?? blank());
      series[s]?.values.push(valueOf(acc, p.agg === 'none' ? 'sum' : p.agg));
    });
  }
  return { xKind: p.xKind, cats, series: pie ? series.slice(0, 1) : series, skipped, dropped };
}

export function niceStep(span: number, count: number): number {
  const raw = span / Math.max(1, count);
  if (!(raw > 0) || !Number.isFinite(raw)) return 1;
  const pow = 10 ** Math.floor(Math.log10(raw));
  const f = raw / pow;
  return (f <= 1 ? 1 : f <= 2 ? 2 : f <= 2.5 ? 2.5 : f <= 5 ? 5 : 10) * pow;
}

export function niceTicks(min: number, max: number, count: number): { lo: number; hi: number; step: number; ticks: number[] } {
  if (min === max) {
    const pad = Math.abs(min) || 1;
    min = min >= 0 && min - pad < 0 ? 0 : min - pad;
    max += pad;
  }
  const step = niceStep(max - min, count);
  const lo = Math.floor(min / step + 1e-9) * step;
  const hi = Math.ceil(max / step - 1e-9) * step;
  const ticks: number[] = [];
  for (let i = 0; lo + i * step <= hi + step / 2 && i < 200; i++) ticks.push(Number((lo + i * step).toPrecision(12)));
  return { lo, hi, step, ticks };
}

const MIN = 60_000;
const HOUR = 60 * MIN;
const DAY = 24 * HOUR;
type TimeStep = { unit: 'second' | 'minute' | 'hour' | 'day' | 'week' | 'month' | 'year'; n: number; ms: number };
const TIME_STEPS: TimeStep[] = [
  { unit: 'second', n: 1, ms: 1000 },
  { unit: 'second', n: 15, ms: 15_000 },
  { unit: 'minute', n: 1, ms: MIN },
  { unit: 'minute', n: 5, ms: 5 * MIN },
  { unit: 'minute', n: 15, ms: 15 * MIN },
  { unit: 'minute', n: 30, ms: 30 * MIN },
  { unit: 'hour', n: 1, ms: HOUR },
  { unit: 'hour', n: 3, ms: 3 * HOUR },
  { unit: 'hour', n: 6, ms: 6 * HOUR },
  { unit: 'hour', n: 12, ms: 12 * HOUR },
  { unit: 'day', n: 1, ms: DAY },
  { unit: 'day', n: 2, ms: 2 * DAY },
  { unit: 'week', n: 1, ms: 7 * DAY },
  { unit: 'month', n: 1, ms: 30 * DAY },
  { unit: 'month', n: 3, ms: 91 * DAY },
  { unit: 'month', n: 6, ms: 182 * DAY },
  { unit: 'year', n: 1, ms: 365 * DAY },
  { unit: 'year', n: 2, ms: 730 * DAY },
  { unit: 'year', n: 5, ms: 1826 * DAY },
  { unit: 'year', n: 10, ms: 3652 * DAY },
  { unit: 'year', n: 25, ms: 9131 * DAY },
  { unit: 'year', n: 100, ms: 36524 * DAY },
];

export interface TimeTick {
  at: number;
  label: string;
}

export function timeTicks(min: number, max: number, count: number, locale: string): TimeTick[] {
  const span = Math.max(1000, max - min);
  const step = TIME_STEPS.find((s) => span / s.ms <= count) ?? TIME_STEPS[TIME_STEPS.length - 1];
  if (!step) return [];
  const d = new Date(min);
  let t: Date;
  if (step.unit === 'year') {
    const y = Math.ceil(d.getUTCFullYear() / step.n) * step.n;
    t = new Date(Date.UTC(y, 0, 1));
    if (t.getTime() < min) t = new Date(Date.UTC(y + step.n, 0, 1));
  }
  else if (step.unit === 'month') {
    const m0 = d.getUTCMonth();
    const m = Math.ceil(m0 / step.n) * step.n;
    t = new Date(Date.UTC(d.getUTCFullYear(), m, 1));
    if (t.getTime() < min) t = new Date(Date.UTC(d.getUTCFullYear(), m + step.n, 1));
  } else if (step.unit === 'week') {
    const day = Date.UTC(d.getUTCFullYear(), d.getUTCMonth(), d.getUTCDate());
    const dow = (new Date(day).getUTCDay() + 6) % 7;
    t = new Date(day - dow * DAY);
    if (t.getTime() < min) t = new Date(t.getTime() + 7 * DAY);
  } else t = new Date(Math.ceil(min / step.ms) * step.ms);
  const out: TimeTick[] = [];
  const fmt = (o: Intl.DateTimeFormatOptions) => new Intl.DateTimeFormat(locale, { timeZone: 'UTC', ...o });
  const year = fmt({ year: 'numeric' });
  const month = fmt({ month: 'short' });
  const monthYear = fmt({ month: 'short', year: 'numeric' });
  const dayMonth = fmt({ day: 'numeric', month: 'short' });
  const time = fmt({ hour: '2-digit', minute: '2-digit', ...(step.unit === 'second' ? { second: '2-digit' } : {}) });
  let lastYear = -1;
  let lastDay = -1;
  for (let i = 0; t.getTime() <= max && i < 400; i++) {
    const at = t.getTime();
    const y = t.getUTCFullYear();
    const dayNo = Math.floor(at / DAY);
    let label: string;
    if (step.unit === 'year') label = year.format(t);
    else if (step.unit === 'month') label = y !== lastYear || t.getUTCMonth() === 0 ? monthYear.format(t) : month.format(t);
    else if (step.unit === 'day' || step.unit === 'week') label = dayMonth.format(t);
    else label = dayNo !== lastDay ? `${dayMonth.format(t)} ${time.format(t)}` : time.format(t);
    out.push({ at, label });
    lastYear = y;
    lastDay = dayNo;
    if (step.unit === 'year') t = new Date(Date.UTC(y + step.n, 0, 1));
    else if (step.unit === 'month') t = new Date(Date.UTC(y, t.getUTCMonth() + step.n, 1));
    else t = new Date(at + step.ms);
  }
  return out;
}

export function dateLabel(locale: string) {
  const f = {
    month: new Intl.DateTimeFormat(locale, { timeZone: 'UTC', month: 'short', year: 'numeric' }),
    day: new Intl.DateTimeFormat(locale, { timeZone: 'UTC', day: 'numeric', month: 'short', year: 'numeric' }),
    minute: new Intl.DateTimeFormat(locale, { timeZone: 'UTC', dateStyle: 'medium', timeStyle: 'short' }),
    second: new Intl.DateTimeFormat(locale, { timeZone: 'UTC', dateStyle: 'medium', timeStyle: 'medium' }),
  };
  return (ms: number, unit: DateUnit) => f[unit === 'year' ? 'month' : unit].format(new Date(ms));
}

const GROUP = { useGrouping: 'always' } as unknown as Intl.NumberFormatOptions;

function compact(maxAbs: number, locale: string): { scale: number; unit: string } {
  const scale = maxAbs >= 1e9 ? 1e9 : maxAbs >= 1e6 ? 1e6 : 1;
  const unit = scale > 1 ? new Intl.NumberFormat(locale, { notation: 'compact' }).format(scale).replace(/^[\d.,\s\u00a0]+/, '') : '';
  return { scale, unit };
}

export function tickFormat(step: number, maxAbs: number, locale: string): (n: number) => string {
  const { scale, unit } = compact(maxAbs, locale);
  const s = step / scale;
  let digits = 0;
  while (digits < 6 && Math.abs(Math.round(s * 10 ** digits) - s * 10 ** digits) > 1e-6) digits++;
  const f = new Intl.NumberFormat(locale, { minimumFractionDigits: digits, maximumFractionDigits: digits, ...GROUP });
  return (n) => (Math.abs(n) < step / 1e6 ? '0' : `${f.format(n / scale)}${unit ? `\u00a0${unit}` : ''}`);
}

export function valueFormat(locale: string): (n: number | null) => string {
  const f = new Intl.NumberFormat(locale, { maximumFractionDigits: 2, ...GROUP });
  return (n) => (n === null ? '—' : f.format(n));
}

export type Role = 'ink' | 'ink-2' | 'ink-3' | 'rule' | 'surface' | `s${number}`;

export type Prim =
  | { t: 'rect'; x: number; y: number; w: number; h: number; fill: Role; opacity?: number }
  | { t: 'line'; x1: number; y1: number; x2: number; y2: number; stroke: Role; width: number; dash?: string }
  | { t: 'path'; d: string; fill?: Role; stroke?: Role; width?: number; opacity?: number }
  | { t: 'circle'; cx: number; cy: number; r: number; fill: Role; stroke?: Role; width?: number; opacity?: number }
  | { t: 'text'; x: number; y: number; text: string; anchor: 'start' | 'middle' | 'end'; fill: Role; size: number; weight?: number; mono?: boolean };

export interface Mark {
  cat: number;
  series: number;
  shape: Prim;
}

export interface Scene {
  width: number;
  height: number;
  hidden?: number;
  prims: Prim[];
  marks: Mark[];
  plot: { x: number; y: number; w: number; h: number };
  pick: { kind: 'x' | 'y'; at: number[] } | { kind: 'pie'; cx: number; cy: number; r0: number; r1: number; angles: number[] } | { kind: 'point'; xs: number[]; ys: number[] };
}

export interface LayoutOptions {
  width: number;
  height: number;
  kind: ChartKind;
  legend: string[];
  locale: string;
  centerLabel?: string;
  stacked?: boolean;
  log?: boolean;
}

export interface ValueScale {
  ticks: number[];
  fmt: (n: number) => string;
  pos: (v: number) => number | null;
  base: number;
  solid: number;
}

export function linearScale(lo: number, hi: number, count: number, locale: string): ValueScale {
  const vt = niceTicks(lo, hi, count);
  const fmt = tickFormat(vt.step, Math.max(Math.abs(vt.lo), Math.abs(vt.hi)), locale);
  return { ticks: vt.ticks, fmt, pos: (v) => (v - vt.lo) / (vt.hi - vt.lo || 1), base: Math.max(vt.lo, Math.min(0, vt.hi)), solid: 0 };
}

export function logScale(lo: number, hi: number, locale: string): ValueScale {
  const min = lo > 0 ? lo : 1;
  const max = Math.max(hi, min);
  const a = Math.floor(Math.log10(min) + 1e-9);
  let b = Math.ceil(Math.log10(max) - 1e-9);
  if (b <= a) b = a + 1;
  const ticks: number[] = [];
  for (let e = a; e <= b; e++) {
    for (const f of b - a <= 2 ? [1, 2, 5] : [1]) {
      const t = Number((f * 10 ** e).toPrecision(12));
      if (t <= 10 ** b * (1 + 1e-9)) ticks.push(t);
    }
  }
  const { scale, unit } = compact(10 ** b, locale);
  const f = new Intl.NumberFormat(locale, { maximumFractionDigits: 6, ...GROUP });
  const span = b - a;
  return { ticks, fmt: (n) => `${f.format(Number((n / scale).toPrecision(6)))}${unit ? `\u00a0${unit}` : ''}`, pos: (v) => (v > 0 ? (Math.log10(v) - a) / span : null), base: 10 ** a, solid: 10 ** a };
}

export function stackOf(series: readonly Series[], n: number): [number, number][][] {
  const up = new Array<number>(n).fill(0);
  const down = new Array<number>(n).fill(0);
  return series.map((s) =>
    Array.from({ length: n }, (_, i) => {
      const v = s.values[i] ?? 0;
      const from = v >= 0 ? (up[i] ?? 0) : (down[i] ?? 0);
      const to = from + v;
      if (v >= 0) up[i] = to;
      else down[i] = to;
      return [from, to] as [number, number];
    }),
  );
}

const FONT = 11;
export const textWidth = (s: string, size = FONT) => s.length * size * 0.58;
const n1 = (v: number) => Math.round(v * 10) / 10;

export function ellipsis(s: string, width: number, size = FONT): string {
  if (textWidth(s, size) <= width) return s;
  const keep = Math.max(1, Math.floor(width / (size * 0.58)) - 1);
  return `${s.slice(0, keep)}…`;
}

const role = (s: number): Role => `s${s % SERIES}`;

function legend(items: string[], x: number, y: number, maxW: number, out: Prim[]): number {
  let cx = x;
  let cy = y;
  for (const [i, raw] of items.entries()) {
    const label = ellipsis(raw, 220);
    const w = 14 + textWidth(label) + 16;
    if (cx > x && cx + w > x + maxW) {
      cx = x;
      cy += 18;
    }
    out.push({ t: 'rect', x: cx, y: cy - 8, w: 9, h: 9, fill: role(i) });
    out.push({ t: 'text', x: cx + 14, y: cy, text: label, anchor: 'start', fill: 'ink-2', size: FONT });
    cx += w;
  }
  return cy - y + 18;
}

function arc(cx: number, cy: number, r0: number, r1: number, a0: number, a1: number): string {
  const full = a1 - a0 >= Math.PI * 2 - 1e-6;
  if (full) a1 = a0 + Math.PI * 2 - 1e-4;
  const p = (r: number, a: number) => `${n1(cx + r * Math.sin(a))} ${n1(cy - r * Math.cos(a))}`;
  const large = a1 - a0 > Math.PI ? 1 : 0;
  return `M${p(r1, a0)}A${r1} ${r1} 0 ${large} 1 ${p(r1, a1)}L${p(r0, a1)}A${r0} ${r0} 0 ${large} 0 ${p(r0, a0)}Z`;
}

function pieScene(data: Prepared, o: LayoutOptions): Scene {
  const prims: Prim[] = [];
  const marks: Mark[] = [];
  const values = (data.series[0]?.values ?? []).map((v) => Math.max(0, v ?? 0));
  const total = values.reduce((a, b) => a + b, 0);
  const pad = 16;
  const side = o.width >= 520;
  const legendW = side ? Math.min(260, o.width * 0.38) : o.width - pad * 2;
  const rows = data.cats.length;
  const legendH = side ? 0 : Math.min(rows, 8) * 18 + 6;
  const r1 = Math.max(24, Math.min((side ? o.width - legendW - pad * 3 : o.width - pad * 2) / 2, (o.height - pad * 2 - legendH) / 2));
  const r0 = r1 * 0.58;
  const cx = side ? pad + r1 : o.width / 2;
  const cy = pad + r1;
  const angles = [0];
  let a = 0;
  const fmt = valueFormat(o.locale);
  const pct = new Intl.NumberFormat(o.locale, { style: 'percent', maximumFractionDigits: 1 });
  values.forEach((v, i) => {
    const a1 = a + (total ? (v / total) * Math.PI * 2 : 0);
    const other = data.cats[i]?.other;
    const shape: Prim = { t: 'path', d: arc(cx, cy, r0, r1, a, a1), fill: other ? 'ink-3' : role(i), stroke: 'surface', width: 1.5, ...(other ? { opacity: 0.45 } : {}) };
    prims.push(shape);
    marks.push({ cat: i, series: 0, shape });
    a = a1;
    angles.push(a);
  });
  prims.push({ t: 'text', x: cx, y: cy + 2, text: fmt(total), anchor: 'middle', fill: 'ink', size: Math.max(12, Math.min(20, r0 / 4)), weight: 600, mono: true });
  if (o.centerLabel) prims.push({ t: 'text', x: cx, y: cy + 18, text: ellipsis(o.centerLabel, r0 * 1.6), anchor: 'middle', fill: 'ink-3', size: FONT });
  const lx = side ? cx + r1 + pad * 2 : pad;
  let ly = side ? Math.max(pad + 10, cy - (Math.min(rows, Math.floor((o.height - pad * 2) / 18)) * 18) / 2 + 10) : cy + r1 + pad + 10;
  const fits = Math.max(1, Math.floor((o.height - ly) / 18));
  data.cats.slice(0, fits).forEach((c, i) => {
    const v = values[i] ?? 0;
    prims.push({ t: 'rect', x: lx, y: ly - 8, w: 9, h: 9, fill: c.other ? 'ink-3' : role(i), ...(c.other ? { opacity: 0.45 } : {}) });
    const share = pct.format(total ? v / total : 0);
    prims.push({ t: 'text', x: lx + legendW, y: ly, text: share, anchor: 'end', fill: 'ink-3', size: FONT, mono: true });
    prims.push({ t: 'text', x: lx + 14, y: ly, text: ellipsis(c.label, legendW - 14 - textWidth(share) - 10), anchor: 'start', fill: 'ink-2', size: FONT });
    ly += 18;
  });
  return { width: o.width, height: o.height, prims, marks, plot: { x: cx - r1, y: cy - r1, w: r1 * 2, h: r1 * 2 }, pick: { kind: 'pie', cx, cy, r0, r1, angles } };
}

export function layout(data: Prepared, o: LayoutOptions): Scene {
  if (o.kind === 'pie') return pieScene(data, o);
  const prims: Prim[] = [];
  const marks: Mark[] = [];
  const pad = 14;
  const lh = legend(o.legend, pad, pad + 9, o.width - pad * 2, prims);
  const top = pad + lh + 6;
  const n = data.cats.length;
  const horizontal = o.kind === 'hbar';
  const timeAxis = continuous(o.kind, data.xKind) && data.xKind === 'date';
  const numAxis = continuous(o.kind, data.xKind) && data.xKind === 'number';

  const stacked = !!o.stacked && canStack(o.kind) && data.series.length > 1;
  const log = !!o.log && canLog(o.kind) && !stacked;
  const stack = stacked ? stackOf(data.series, n) : [];
  let lo = Infinity;
  let hi = -Infinity;
  let hidden = 0;
  const see = (v: number) => {
    if (log && v <= 0) return void hidden++;
    lo = Math.min(lo, v);
    hi = Math.max(hi, v);
  };
  if (stacked) for (const s of stack) for (const [a, b] of s) [a, b].forEach(see);
  else for (const s of data.series) for (const v of s.values) if (v !== null) see(v);
  if (lo === Infinity) {
    lo = log ? 1 : 0;
    hi = log ? 10 : 1;
  }
  if (!log && (o.kind === 'bar' || o.kind === 'hbar' || o.kind === 'area')) {
    lo = Math.min(0, lo);
    hi = Math.max(0, hi);
  }
  const scaleOf = (count: number) => (log ? logScale(lo, hi, o.locale) : linearScale(lo, hi, count, o.locale));
  const done = (scene: Scene): Scene => (hidden ? { ...scene, hidden } : scene);

  const height = horizontal ? Math.max(o.height, top + n * 16 + 30) : o.height;
  const bottom = height - pad - 18;

  if (horizontal) {
    const labelW = Math.min(Math.max(...data.cats.map((c) => textWidth(c.label)), 20) + 6, o.width * 0.32);
    const left = pad + labelW + 6;
    const plotW = o.width - left - pad - 8;
    const vs = scaleOf(Math.max(2, Math.floor(plotW / 90)));
    const X = (v: number) => left + (vs.pos(v) ?? 0) * plotW;
    for (const t of vs.ticks) {
      prims.push({ t: 'line', x1: n1(X(t)), y1: top, x2: n1(X(t)), y2: bottom, stroke: 'rule', width: 1, ...(t === vs.solid ? {} : { dash: '2 3' }) });
      const text = vs.fmt(t);
      const end = X(t) + textWidth(text) / 2 > o.width - 2;
      prims.push({ t: 'text', x: n1(X(t)), y: bottom + 14, text, anchor: end ? 'end' : 'middle', fill: 'ink-3', size: FONT, mono: true });
    }
    const band = (bottom - top) / Math.max(1, n);
    const lanes = stacked ? 1 : data.series.length;
    const inner = Math.min(band * 0.72, 28 * lanes);
    const bw = inner / Math.max(1, lanes);
    const at: number[] = [];
    data.cats.forEach((c, i) => {
      const y0 = top + i * band;
      at.push(y0 + band / 2);
      if (band >= 11 || i % Math.ceil(11 / band) === 0) prims.push({ t: 'text', x: left - 8, y: n1(y0 + band / 2 + 4), text: ellipsis(c.label, labelW), anchor: 'end', fill: c.other ? 'ink-3' : 'ink-2', size: FONT });
      data.series.forEach((s, k) => {
        const v = s.values[i];
        if (v === null || v === undefined || (log && v <= 0)) return;
        const seg = stack[k]?.[i];
        const a = X(seg ? seg[0] : vs.base);
        const b = X(seg ? seg[1] : v);
        const lane = stacked ? 0 : k;
        const shape: Prim = { t: 'rect', x: n1(Math.min(a, b)), y: n1(y0 + (band - inner) / 2 + lane * bw), w: n1(Math.max(1, Math.abs(b - a))), h: n1(Math.max(1, bw - (lanes > 1 ? 1 : 0))), fill: role(k), ...(c.other ? { opacity: 0.45 } : {}) };
        prims.push(shape);
        marks.push({ cat: i, series: k, shape });
      });
    });
    return done({ width: o.width, height, prims, marks, plot: { x: left, y: top, w: plotW, h: bottom - top }, pick: { kind: 'y', at } });
  }

  const vs = scaleOf(Math.max(2, Math.floor((bottom - top) / 44)));
  const left = pad + Math.max(...vs.ticks.map((t) => textWidth(vs.fmt(t)))) + 8;
  const right = o.width - pad - 6;
  const plotW = right - left;
  const Y = (v: number) => bottom - (vs.pos(v) ?? 0) * (bottom - top);
  for (const t of vs.ticks) {
    prims.push({ t: 'line', x1: left, y1: n1(Y(t)), x2: right, y2: n1(Y(t)), stroke: 'rule', width: 1, ...(t === vs.solid ? {} : { dash: '2 3' }) });
    prims.push({ t: 'text', x: left - 6, y: n1(Y(t) + 4), text: vs.fmt(t), anchor: 'end', fill: 'ink-3', size: FONT, mono: true });
  }

  let X: (i: number) => number;
  if (timeAxis || numAxis) {
    const xs = data.cats.map((c) => c.at ?? 0);
    let xlo = Math.min(...xs);
    let xhi = Math.max(...xs);
    if (!xs.length) xlo = xhi = 0;
    const count = Math.max(2, Math.floor(plotW / 96));
    let ticks: TimeTick[];
    if (timeAxis) {
      if (xlo === xhi) {
        xlo -= DAY;
        xhi += DAY;
      }
      ticks = timeTicks(xlo, xhi, count, o.locale);
    } else {
      const nt = niceTicks(xlo, xhi, count);
      xlo = nt.lo;
      xhi = nt.hi;
      const f = tickFormat(nt.step, Math.max(Math.abs(xlo), Math.abs(xhi)), o.locale);
      ticks = nt.ticks.map((at) => ({ at, label: f(at) }));
    }
    const span = xhi - xlo || 1;
    const XV = (v: number) => left + ((v - xlo) / span) * plotW;
    X = (i: number) => XV(data.cats[i]?.at ?? 0);
    let lastEnd = -Infinity;
    for (const t of ticks) {
      const x = XV(t.at);
      const w = textWidth(t.label);
      if (t.at < xlo || t.at > xhi || x - w / 2 < lastEnd + 8 || x - w / 2 < 4 || x + w / 2 > o.width - 2) continue;
      prims.push({ t: 'line', x1: n1(x), y1: bottom, x2: n1(x), y2: bottom + 4, stroke: 'ink-3', width: 1 });
      prims.push({ t: 'text', x: n1(x), y: bottom + 16, text: t.label, anchor: 'middle', fill: 'ink-3', size: FONT, mono: !timeAxis });
      lastEnd = x + w / 2;
    }
  } else {
    const band = plotW / Math.max(1, n);
    X = (i: number) => left + band * (i + 0.5);
    const every = Math.max(1, Math.ceil(Math.min(90, Math.max(...data.cats.map((c) => textWidth(c.label)), 10) + 10) / band));
    data.cats.forEach((c, i) => {
      if (i % every) return;
      prims.push({ t: 'text', x: n1(X(i)), y: bottom + 16, text: ellipsis(c.label, Math.max(band * every - 8, 24)), anchor: 'middle', fill: c.other ? 'ink-3' : 'ink-2', size: FONT });
    });
  }
  prims.push({ t: 'line', x1: left, y1: n1(Y(vs.base)), x2: right, y2: n1(Y(vs.base)), stroke: 'ink-3', width: 1 });

  const at = data.cats.map((_, i) => X(i));
  if (o.kind === 'bar') {
    const band = plotW / Math.max(1, n);
    const lanes = stacked ? 1 : data.series.length;
    const inner = Math.min(band * 0.74, 48 * lanes);
    const bw = inner / Math.max(1, lanes);
    data.series.forEach((s, k) =>
      s.values.forEach((v, i) => {
        if (v === null || (log && v <= 0)) return;
        const seg = stack[k]?.[i];
        const base = Y(seg ? seg[0] : vs.base);
        const y = Y(seg ? seg[1] : v);
        const lane = stacked ? 0 : k;
        const shape: Prim = { t: 'rect', x: n1(X(i) - inner / 2 + lane * bw), y: n1(Math.min(y, base)), w: n1(Math.max(1, bw - (lanes > 1 && bw > 4 ? 1 : 0))), h: n1(Math.max(1, Math.abs(base - y))), fill: role(k), ...(data.cats[i]?.other ? { opacity: 0.45 } : {}) };
        prims.push(shape);
        marks.push({ cat: i, series: k, shape });
      }),
    );
    return done({ width: o.width, height, prims, marks, plot: { x: left, y: top, w: plotW, h: bottom - top }, pick: { kind: 'x', at } });
  }

  if (o.kind === 'scatter') {
    const xs: number[] = [];
    const ys: number[] = [];
    data.series.forEach((s, k) =>
      s.values.forEach((v, i) => {
        if (v === null || (log && v <= 0)) return;
        const shape: Prim = { t: 'circle', cx: n1(X(i)), cy: n1(Y(v)), r: 3, fill: role(k), opacity: 0.78 };
        prims.push(shape);
        marks.push({ cat: i, series: k, shape });
        if (k === 0) {
          xs[i] = X(i);
          ys[i] = Y(v);
        }
      }),
    );
    return done({ width: o.width, height, prims, marks, plot: { x: left, y: top, w: plotW, h: bottom - top }, pick: { kind: 'point', xs: at, ys: data.cats.map((_, i) => ys[i] ?? -1e9) } });
  }

  const base = Y(vs.base);
  const dots = n <= 60;
  data.series.forEach((s, k) => {
    const runs: string[] = [];
    const fills: string[] = [];
    let run: [number, number][] = [];
    const flush = () => {
      if (!run.length) return;
      runs.push(run.map(([x, y], j) => `${j ? 'L' : 'M'}${n1(x)} ${n1(y)}`).join(''));
      const first = run[0];
      const last = run[run.length - 1];
      if (first && last) fills.push(`${runs[runs.length - 1]}L${n1(last[0])} ${n1(base)}L${n1(first[0])} ${n1(base)}Z`);
      run = [];
    };
    s.values.forEach((v, i) => {
      if (v === null || (log && v <= 0)) return flush();
      run.push([X(i), Y(v)]);
    });
    flush();
    if (o.kind === 'area') for (const d of fills) prims.push({ t: 'path', d, fill: role(k), opacity: 0.16 });
    for (const d of runs) prims.push({ t: 'path', d, stroke: role(k), width: 1.75 });
    s.values.forEach((v, i) => {
      if (v === null || (log && v <= 0)) return;
      const shape: Prim = { t: 'circle', cx: n1(X(i)), cy: n1(Y(v)), r: dots ? 2.5 : 0, fill: role(k) };
      if (dots) prims.push(shape);
      marks.push({ cat: i, series: k, shape: { ...shape, r: 4 } });
    });
  });
  return done({ width: o.width, height, prims, marks, plot: { x: left, y: top, w: plotW, h: bottom - top }, pick: { kind: 'x', at } });
}

export function hit(scene: Scene, x: number, y: number): number | null {
  const p = scene.pick;
  if (p.kind === 'pie') {
    const dx = x - p.cx;
    const dy = y - p.cy;
    const r = Math.hypot(dx, dy);
    if (r < p.r0 - 2 || r > p.r1 + 4) return null;
    const a = (Math.atan2(dx, -dy) + Math.PI * 2) % (Math.PI * 2);
    for (let i = 0; i < p.angles.length - 1; i++) if (a >= (p.angles[i] ?? 0) && a < (p.angles[i + 1] ?? 0)) return i;
    return null;
  }
  const pl = scene.plot;
  if (x < pl.x - 8 || x > pl.x + pl.w + 8 || y < pl.y - 8 || y > pl.y + pl.h + 8) return null;
  if (p.kind === 'point') {
    let best: number | null = null;
    let d = 18 * 18;
    p.xs.forEach((px, i) => {
      const e = (px - x) ** 2 + ((p.ys[i] ?? -1e9) - y) ** 2;
      if (e < d) {
        d = e;
        best = i;
      }
    });
    return best;
  }
  const v = p.kind === 'x' ? x : y;
  let best: number | null = null;
  let d = Infinity;
  p.at.forEach((a, i) => {
    const e = Math.abs(a - v);
    if (e < d) {
      d = e;
      best = i;
    }
  });
  return best;
}

export type Colors = Record<string, string>;

const esc = (s: string) => s.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;').replace(/"/g, '&quot;');

export function toSvg(scene: Scene, colors: Colors, fonts: { ui: string; mono: string }): string {
  const c = (r: Role | undefined) => (r ? (colors[r] ?? '#000') : 'none');
  const out = [`<svg xmlns="http://www.w3.org/2000/svg" width="${scene.width}" height="${scene.height}" viewBox="0 0 ${scene.width} ${scene.height}">`, `<rect width="100%" height="100%" fill="${c('surface')}"/>`];
  for (const p of scene.prims) {
    const op = 'opacity' in p && p.opacity !== undefined ? ` fill-opacity="${p.opacity}"` : '';
    if (p.t === 'rect') out.push(`<rect x="${p.x}" y="${p.y}" width="${p.w}" height="${p.h}" fill="${c(p.fill)}"${op}/>`);
    else if (p.t === 'line') out.push(`<line x1="${p.x1}" y1="${p.y1}" x2="${p.x2}" y2="${p.y2}" stroke="${c(p.stroke)}" stroke-width="${p.width}"${p.dash ? ` stroke-dasharray="${p.dash}"` : ''}/>`);
    else if (p.t === 'path') out.push(`<path d="${p.d}" fill="${c(p.fill)}" stroke="${c(p.stroke)}"${p.width ? ` stroke-width="${p.width}"` : ''} stroke-linejoin="round"${op}/>`);
    else if (p.t === 'circle') out.push(`<circle cx="${p.cx}" cy="${p.cy}" r="${p.r}" fill="${c(p.fill)}"${p.stroke ? ` stroke="${c(p.stroke)}" stroke-width="${p.width ?? 1}"` : ''}${op}/>`);
    else out.push(`<text x="${p.x}" y="${p.y}" text-anchor="${p.anchor}" font-family="${esc(p.mono ? fonts.mono : fonts.ui)}" font-size="${p.size}"${p.weight ? ` font-weight="${p.weight}"` : ''} fill="${c(p.fill)}">${esc(p.text)}</text>`);
  }
  out.push('</svg>');
  return out.join('');
}

export function paint(ctx: CanvasRenderingContext2D, scene: Scene, colors: Colors, fonts: { ui: string; mono: string }) {
  const c = (r: Role) => colors[r] ?? '#000';
  ctx.fillStyle = c('surface');
  ctx.fillRect(0, 0, scene.width, scene.height);
  for (const p of scene.prims) {
    ctx.save();
    if (p.t === 'rect') {
      ctx.globalAlpha = p.opacity ?? 1;
      ctx.fillStyle = c(p.fill);
      ctx.fillRect(p.x, p.y, p.w, p.h);
    } else if (p.t === 'line') {
      ctx.strokeStyle = c(p.stroke);
      ctx.lineWidth = p.width;
      if (p.dash) ctx.setLineDash(p.dash.split(' ').map(Number));
      ctx.beginPath();
      ctx.moveTo(p.x1, p.y1);
      ctx.lineTo(p.x2, p.y2);
      ctx.stroke();
    } else if (p.t === 'path') {
      const path = new Path2D(p.d);
      if (p.fill) {
        ctx.globalAlpha = p.opacity ?? 1;
        ctx.fillStyle = c(p.fill);
        ctx.fill(path);
        ctx.globalAlpha = 1;
      }
      if (p.stroke) {
        ctx.strokeStyle = c(p.stroke);
        ctx.lineWidth = p.width ?? 1;
        ctx.lineJoin = 'round';
        ctx.stroke(path);
      }
    } else if (p.t === 'circle') {
      ctx.globalAlpha = p.opacity ?? 1;
      ctx.beginPath();
      ctx.arc(p.cx, p.cy, p.r, 0, Math.PI * 2);
      ctx.fillStyle = c(p.fill);
      ctx.fill();
      if (p.stroke) {
        ctx.strokeStyle = c(p.stroke);
        ctx.lineWidth = p.width ?? 1;
        ctx.stroke();
      }
    } else {
      ctx.fillStyle = c(p.fill);
      ctx.font = `${p.weight ?? 400} ${p.size}px ${p.mono ? fonts.mono : fonts.ui}`;
      ctx.textAlign = p.anchor === 'middle' ? 'center' : p.anchor === 'end' ? 'right' : 'left';
      ctx.fillText(p.text, p.x, p.y);
    }
    ctx.restore();
  }
}

export function fileName(base: string, ext: 'svg' | 'png'): string {
  const slug = base
    .normalize('NFKD')
    .replace(/[̀-ͯ]/g, '')
    .replace(/[^\w-]+/g, '-')
    .replace(/^-+|-+$/g, '')
    .toLowerCase()
    .slice(0, 60);
  return `${slug || m.file_chart()}.${ext}`;
}
