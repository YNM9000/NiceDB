import { prepare, type ChartColumn, type ChartSettings, type PrepareInput, type Prepared } from './chart';

export const OFF_THREAD = 50_000;

export const charts = $state<{ settings: Record<string, ChartSettings>; open: Record<string, boolean>; picked: Record<string, Picked | null>; full: Record<string, boolean> }>({ settings: {}, open: {}, picked: {}, full: {} });

type Picked = { columns: ChartColumn[]; rows: (string | null)[][] };

export function pickFrom(sel: Picked | undefined, rows: number, columns: number): Picked | null {
  return sel && sel.rows.length > 1 && (sel.rows.length < rows || sel.columns.length < columns) ? sel : null;
}

let worker: Worker | null = null;
let seq = 0;
const waiting = new Map<number, (p: Prepared) => void>();

function spawn(): Worker | null {
  if (worker || typeof Worker === 'undefined') return worker;
  worker = new Worker(new URL('./chart.worker.ts', import.meta.url), { type: 'module' });
  worker.onmessage = (e: MessageEvent<{ seq: number; data: Prepared }>) => {
    waiting.get(e.data.seq)?.(e.data.data);
    waiting.delete(e.data.seq);
  };
  return worker;
}

export function prepareLater(input: PrepareInput): Promise<Prepared> | Prepared {
  const w = input.xs.length > OFF_THREAD ? spawn() : null;
  if (!w) return prepare(input);
  const mine = ++seq;
  for (const k of waiting.keys()) if (k < mine) waiting.delete(k);
  return new Promise((resolve) => {
    waiting.set(mine, resolve);
    w.postMessage({ seq: mine, input });
  });
}

