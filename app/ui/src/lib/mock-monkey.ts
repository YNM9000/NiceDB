import { ApiError, type Api, type RunEvent, type RunRequest } from './api';

type Gen = { columns: [string, string][]; total: number; row: (i: number) => (string | null)[] };

export type Net = 'ok' | 'cade' | 'appesa' | 'lenta';

const NESTED = `${'{"a":['.repeat(400)}1${']}'.repeat(400)}`;
const RTL = 'שלום עולם — مرحبا بالعالم 123 abc';
const EMOJI = '👩‍👩‍👧‍👦🏳️‍🌈🇮🇹 e̊̃͂ ∑ 中文 𝔘𝔫𝔦𝔠𝔬𝔡𝔢';
export const LONG_NAME = `colonna_${'lunghissima_'.repeat(833)}`.slice(0, 10_000);
export const LONG_TABLE = `tabella_${'lunghissima_'.repeat(833)}`.slice(0, 10_000);

const wideRows = new Map<number, (string | null)[]>();

export function monkeyTables(params: URLSearchParams): Record<string, Gen> {
  if (!params.has('monkey')) return {};
  const huge = 'x'.repeat(10 * 1024 * 1024);
  const wide = Array.from({ length: 499 }, (_, k): [string, string] => [`c${String(k).padStart(3, '0')}`, k % 2 ? 'text' : 'int4']);
  return {
    monkey_values: {
      columns: [['id', 'int4'], ['enorme', 'text'], ['annidato', 'jsonb'], ['rtl', 'text'], ['emoji', 'text'], ['vuoto', 'text'], ['nullo', 'text'], ['binario', 'bytea'], [LONG_NAME, 'text']],
      total: 400,
      row: (i) => [String(i), i === 1 ? huge : `riga ${i}`, i % 2 ? NESTED : '{}', RTL, EMOJI, '', null, i % 3 ? null : `\\x${'00ff'.repeat(i === 3 ? 2_000_000 : 8)}`, i % 5 ? null : `${i}`],
    },
    monkey_nulls: {
      columns: [['id', 'int4'], ['a', 'text'], ['b', 'numeric'], ['c', 'jsonb'], ['d', 'bytea'], ['e', 'date']],
      total: 200,
      row: (i) => [String(i), null, null, null, null, null],
    },
    [LONG_TABLE]: {
      columns: [['id', 'int4'], [LONG_NAME, 'text']],
      total: 3,
      row: (i) => [String(i), 'x'],
    },
    monkey_wide: {
      columns: [['id', 'int4'], ...wide],
      total: 2000,
      row: (i) => {
        let r = wideRows.get(i);
        if (!r) wideRows.set(i, (r = [String(i), ...wide.map(([, t], k) => (t === 'int4' ? String(i * k) : `v${i}.${k}`))]));
        return r;
      },
    },
  };
}

const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));
const NETWORKED = ['sessionOpen', 'sessionSwitch', 'sessionReconnect', 'schemaLoad', 'objectDdl', 'dataBrowse', 'dataApply', 'dataPreview', 'dataEstimate', 'dataRelated', 'keysScan', 'keyRead', 'explain', 'connectionTest'] as const;
const dropped = () => new ApiError({ code: 'driver', detail: { kind: 'io', message: 'connection reset by peer' } });

export function chaosNet(base: Api, initial: string | null): Partial<Api> {
  let net: Net = initial === 'cade' || initial === 'appesa' || initial === 'lenta' ? initial : 'ok';
  const waiting = new Set<() => void>();
  const runs = new Map<string, () => void>();
  const release = () => {
    const all = [...waiting].reverse();
    waiting.clear();
    for (const w of all) w();
  };
  if (typeof window !== 'undefined')
    Object.assign(window, {
      __ndRete: (next: Net) => {
        net = next;
        if (next !== 'appesa') release();
      },
    });
  const gate = async () => {
    if (net === 'lenta') await sleep(1500);
    if (net === 'appesa') await new Promise<void>((r) => waiting.add(r));
    if (net === 'cade') throw dropped();
  };
  const out: Record<string, unknown> = {};
  for (const name of NETWORKED) {
    const fn = base[name] as (...a: unknown[]) => Promise<unknown>;
    out[name] = async (...args: unknown[]) => {
      await gate();
      return fn(...args);
    };
  }
  const run = async (req: RunRequest, emit: (e: RunEvent) => void) => {
    if (net === 'ok') return base.run(req, emit);
    if (net === 'lenta') {
      await sleep(1500);
      return base.run(req, emit);
    }
    emit({ type: 'statement', index: 0, from: 0, to: req.text.length, sql: req.text, unsortable: null });
    if (net === 'appesa') {
      const cancelled = await new Promise<boolean>((r) => {
        runs.set(req.run_id, () => r(true));
        waiting.add(() => r(false));
      });
      runs.delete(req.run_id);
      if (!cancelled) return base.run(req, emit);
      emit({ type: 'failed', index: 0, elapsed_ms: 0, error: { kind: 'cancelled' }, position: null, at: null });
      emit({ type: 'finished', elapsed_ms: 0, cancelled: true });
      return;
    }
    emit({ type: 'failed', index: 0, elapsed_ms: 3, error: { kind: 'io', message: 'connection reset by peer' }, position: null, at: null });
    emit({ type: 'finished', elapsed_ms: 3, cancelled: false });
  };
  const runCancel = async (id: string) => {
    const stop = runs.get(id);
    if (stop) return stop();
    return base.runCancel(id);
  };
  return { ...(out as Partial<Api>), run, runCancel };
}

export function chaosCalls(base: Api, seed: string | null): Partial<Api> {
  if (seed === null) return {};
  let s = Number(seed) >>> 0 || 1;
  const next = () => {
    s = (s + 0x6d2b79f5) >>> 0;
    let t = s;
    t = Math.imul(t ^ (t >>> 15), t | 1);
    t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
  const out: Record<string, unknown> = {};
  for (const name of NETWORKED) {
    const fn = base[name] as (...a: unknown[]) => Promise<unknown>;
    out[name] = async (...args: unknown[]) => {
      if (next() < 0.2) throw next() < 0.5 ? dropped() : new ApiError({ code: 'driver', detail: { kind: 'query', message: 'caos: errore inventato', code: null, detail: null, hint: null, offset: null } });
      return fn(...args);
    };
  }
  return out as Partial<Api>;
}
