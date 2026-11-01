import { m } from '../paraglide/messages';
import { aggregate, formatAvg, formatDecimal, type Column, type Stats } from './aggregate';
import { num, tag } from './format';

export const OFF_THREAD = 20_000;

export interface Wider {
  loaded: number;
  run: () => Promise<Stats>;
}

export const selectionStats = $state<{ stats: Stats | null; busy: boolean; wider: Wider | null; all: boolean; error: string | null }>({ stats: null, busy: false, wider: null, all: false, error: null });

let owner: object | null = null;
let seq = 0;
let worker: Worker | null = null;

function spawn(): Worker | null {
  if (worker || typeof Worker === 'undefined') return worker;
  worker = new Worker(new URL('./aggregate.worker.ts', import.meta.url), { type: 'module' });
  worker.onmessage = (e: MessageEvent<{ seq: number; stats: Stats }>) => {
    if (e.data.seq !== seq) return;
    selectionStats.stats = e.data.stats;
    selectionStats.busy = false;
  };
  return worker;
}

export function publish(from: object, cols: Column[] | null, wider: Wider | null = null) {
  owner = from;
  const mine = ++seq;
  selectionStats.wider = wider;
  selectionStats.all = false;
  selectionStats.error = null;
  const cells = cols?.reduce((n, c) => n + c.values.length, 0) ?? 0;
  if (!cols || cells < 2) {
    selectionStats.stats = null;
    selectionStats.busy = false;
    return;
  }
  const w = cells > OFF_THREAD ? spawn() : null;
  if (!w) {
    selectionStats.stats = aggregate(cols);
    selectionStats.busy = false;
    return;
  }
  selectionStats.busy = true;
  w.postMessage({ seq: mine, cols });
}

export function retract(from: object) {
  if (owner !== from) return;
  owner = null;
  seq++;
  selectionStats.stats = null;
  selectionStats.busy = false;
  selectionStats.wider = null;
}

export async function widen() {
  const w = selectionStats.wider;
  if (!w || selectionStats.busy) return;
  const mine = ++seq;
  selectionStats.busy = true;
  selectionStats.error = null;
  try {
    const stats = await w.run();
    if (mine !== seq) return;
    selectionStats.stats = stats;
    selectionStats.all = true;
  } catch (e) {
    if (mine === seq) selectionStats.error = e instanceof Error ? e.message : String(e);
  } finally {
    if (mine === seq) selectionStats.busy = false;
  }
}

export interface StatPart {
  key: string;
  label: string;
  value: string;
  raw: string;
}

export function statParts(s: Stats): StatPart[] {
  const loc = tag();
  const out: StatPart[] = [{ key: 'count', label: m.agg_count(), value: num(s.count), raw: String(s.count) }];
  if (s.numeric && s.sum !== null) {
    out.push({ key: 'sum', label: m.agg_sum(), value: formatDecimal(s.sum, loc), raw: s.sum });
    if (s.avg !== null) out.push({ key: 'avg', label: m.agg_avg(), value: formatAvg(s.avg, s.scale, loc), raw: String(s.avg) });
    if (s.min !== null) out.push({ key: 'min', label: m.agg_min(), value: formatDecimal(s.min, loc), raw: s.min });
    if (s.max !== null) out.push({ key: 'max', label: m.agg_max(), value: formatDecimal(s.max, loc), raw: s.max });
  } else if (s.distinct !== null) out.push({ key: 'distinct', label: m.agg_distinct(), value: num(s.distinct), raw: String(s.distinct) });
  if (s.nulls) out.push({ key: 'nulls', label: 'NULL', value: num(s.nulls), raw: String(s.nulls) });
  return out;
}
