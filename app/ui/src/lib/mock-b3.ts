import { ApiError, type Api, type CommitMode, type ConnectionProfile, type RowPreview, type RunEvent, type RunRequest, type TxState, type WriteAccess } from './api';
import { readPrefs } from './mock-notes';

type Run = (req: RunRequest, emit: (e: RunEvent) => void) => Promise<void>;
type Gen = { columns: [string, string][]; total: number; row: (i: number) => (string | null)[] };

interface Tx {
  mode: CommitMode;
  open: boolean;
  writes: number;
  rows: number;
  began: number;
  dry: boolean;
  failed: boolean;
}

const WRITE = /^\s*(insert|update|delete|merge|create|alter|drop|truncate)\b/i;
const OUTSIDE = /^\s*(vacuum|begin|start|commit|rollback|end|abort|create\s+database|drop\s+database)\b/i;
const SHOWN = 200;

function statements(text: string) {
  const out: { from: number; to: number; sql: string }[] = [];
  let start = 0;
  for (let i = 0; i <= text.length; i++) {
    if (i < text.length && text[i] !== ';') continue;
    const raw = text.slice(start, i);
    const sql = raw.trim();
    const from = start + raw.length - raw.trimStart().length;
    if (sql) out.push({ from, to: from + sql.length, sql });
    start = i + 1;
  }
  return out;
}

function pick(req: RunRequest) {
  const all = statements(req.text);
  const s = req.scope;
  if (s.kind === 'script') return all;
  if (s.kind === 'selection') return all.filter((r) => r.to > Math.min(s.from, s.to) && r.from < Math.max(s.from, s.to));
  const at = [...all].reverse().find((r) => r.from <= s.cursor) ?? all[0];
  return at ? [at] : [];
}

const literal = (v: string) => v.trim().replace(/^'(.*)'$/s, '$1');

function matching(gen: Gen, where: string | undefined) {
  const names = gen.columns.map(([n]) => n);
  const scan = Math.min(gen.total, 2400);
  const all = Array.from({ length: scan }, (_, i) => gen.row(i + 1));
  if (!where) return { rows: all, total: gen.total };
  const m = /^\s*(\w+)\s*(=|<|>|in)\s*(.+?)\s*$/is.exec(where);
  const at = names.indexOf(m?.[1] ?? '');
  if (!m || at < 0) return { rows: all.slice(0, 3), total: 3 };
  const op = (m[2] ?? '=').toLowerCase();
  const raw = m[3] ?? '';
  const wanted = op === 'in' ? raw.replace(/^\(|\)$/g, '').split(',').map(literal) : [literal(raw)];
  const rows = all.filter((r) => {
    const v = r[at] ?? '';
    if (op === '<') return Number(v) < Number(wanted[0]);
    if (op === '>') return Number(v) > Number(wanted[0]);
    return wanted.includes(v);
  });
  return { rows, total: rows.length };
}

function evaluate(expr: string, row: (string | null)[], names: string[]): string | null {
  const m = /^\s*(\w+)\s*([*+-])\s*([\d.]+)\s*$/.exec(expr);
  if (m) {
    const base = Number(row[names.indexOf(m[1] ?? '')] ?? 0);
    const n = Number(m[3]);
    const v = m[2] === '*' ? base * n : m[2] === '+' ? base + n : base - n;
    return v.toFixed(2);
  }
  return /^null$/i.test(expr.trim()) ? null : literal(expr);
}

function preview(sql: string, tables: Record<string, Gen>): { preview: RowPreview | null; affected: number } {
  const upd = /^\s*update\s+(?:public\.)?(\w+)\s+set\s+(.+?)(?:\s+where\s+(.+))?$/is.exec(sql);
  const del = /^\s*delete\s+from\s+(?:public\.)?(\w+)(?:\s+where\s+(.+))?$/is.exec(sql);
  const name = upd?.[1] ?? del?.[1] ?? '';
  const gen = tables[name];
  if (!gen || (!upd && !del)) return { preview: null, affected: /^\s*insert/i.test(sql) ? 1 : 3 };
  const names = gen.columns.map(([n]) => n);
  const { rows, total } = matching(gen, upd ? upd[3] : del?.[2]);
  const before = rows.slice(0, SHOWN);
  const columns = gen.columns.map(([n, t]) => ({ name: n, type_name: t }));
  if (!upd) return { preview: { kind: 'delete', columns, before, after: [], changed: [], truncated: total > SHOWN, measured: true }, affected: total };
  const sets = (upd[2] ?? '').split(/,(?![^(]*\))/).map((p) => p.split('=').map((x) => x.trim()) as [string, string]);
  const changed = sets.map(([c]) => names.indexOf(c)).filter((i) => i >= 0);
  const after = before.map((r) => {
    const next = [...r];
    for (const [c, e] of sets) {
      const i = names.indexOf(c);
      if (i >= 0) next[i] = evaluate(e, r, names);
    }
    return next;
  });
  return { preview: { kind: 'update', columns, before, after, changed, truncated: total > SHOWN, measured: !/\b(now|random)\s*\(/i.test(sql) }, affected: total };
}

export function txMock(opts: { tables: Record<string, Gen>; profile: (sid: string) => ConnectionProfile | undefined; access: (sid: string) => Promise<WriteAccess> }) {
  const txs = new Map<string, Tx>();
  const of = (sid: string): Tx => {
    let t = txs.get(sid);
    if (!t) txs.set(sid, (t = { mode: 'auto', open: false, writes: 0, rows: 0, began: 0, dry: false, failed: false }));
    return t;
  };
  const state = (sid: string): TxState => {
    const t = of(sid);
    return { mode: t.mode, open: t.open, writes: t.open ? t.writes : 0, rows: t.open ? t.rows : 0, open_ms: t.open ? Date.now() - t.began : null, failed: t.open && t.failed, dry_run: t.open && t.dry };
  };
  const begin = (t: Tx, dry: boolean) => Object.assign(t, { open: true, writes: 0, rows: 0, began: Date.now(), dry, failed: false });
  const end = (t: Tx) => Object.assign(t, { open: false, writes: 0, rows: 0, dry: false, failed: false });
  const production = (sid: string) => opts.profile(sid)?.environment === 'production';
  if (typeof window !== 'undefined') Object.assign(window, { __ndTx: (sid: string) => state(sid) });

  async function dryRun(req: RunRequest, emit: (e: RunEvent) => void) {
    const t = of(req.session_id);
    if (t.open) throw new ApiError({ code: 'transaction_open' });
    const list = pick(req);
    const bad = list.findIndex((st) => OUTSIDE.test(st.sql));
    const refused = list[bad];
    if (refused) throw new ApiError({ code: 'not_dry_runnable', detail: { index: bad, sql: refused.sql } });
    if (production(req.session_id) && !(await opts.access(req.session_id)).open) throw new ApiError({ code: 'read_only' });
    begin(t, true);
    const started = performance.now();
    for (const [index, st] of list.entries()) {
      emit({ type: 'statement', index, from: st.from, to: st.to, sql: st.sql, unsortable: null });
      if (!WRITE.test(st.sql)) {
        emit({ type: 'columns', index, columns: [{ name: '?column?', type_name: 'int4' }] });
        emit({ type: 'rows', index, rows: [['1']] });
        emit({ type: 'done', index, elapsed_ms: 1, row_count: 1, rows_affected: null, command: 'SELECT', truncated: false, row_limit: null });
        continue;
      }
      const { preview: p, affected } = preview(st.sql, opts.tables);
      const insert = /^\s*insert\s+into\s+(?:public\.)?(\w+)/i.exec(st.sql);
      const gen = insert ? opts.tables[insert[1] ?? ''] : undefined;
      if (gen) {
        emit({ type: 'columns', index, columns: gen.columns.map(([name, type_name]) => ({ name, type_name })) });
        emit({ type: 'rows', index, rows: [gen.row(gen.total + 1)] });
      }
      const command = st.sql.trim().split(/\s+/)[0]?.toUpperCase() ?? null;
      emit({ type: 'done', index, elapsed_ms: 4, row_count: gen ? 1 : 0, rows_affected: affected, command, truncated: false, row_limit: null });
      if (p) emit({ type: 'preview', index, preview: p });
      t.writes++;
      t.rows += affected;
    }
    emit({ type: 'transaction', state: state(req.session_id) });
    emit({ type: 'finished', elapsed_ms: Math.round(performance.now() - started), cancelled: false });
  }

  function wrap(run: Run): Run {
    return async (req, emit) => {
      if (req.dry_run && pick(req).some((st) => WRITE.test(st.sql))) return dryRun(req, emit);
      const t = of(req.session_id);
      const sql = new Map<number, string>();
      await run(req, (e) => {
        if (e.type === 'statement') {
          sql.set(e.index, e.sql);
          const write = WRITE.test(e.sql);
          if (!t.open && (t.mode === 'manual' || (t.mode === 'smart' && write))) begin(t, false);
        }
        if (e.type === 'done' && t.open && WRITE.test(sql.get(e.index) ?? '')) {
          t.writes++;
          t.rows += e.rows_affected ?? 0;
        }
        if (e.type === 'failed' && t.open) t.failed = true;
        if (e.type === 'finished') emit({ type: 'transaction', state: state(req.session_id) });
        emit(e);
      });
    };
  }

  const api: Pick<Api, 'txState' | 'txModeSet' | 'txCommit' | 'txRollback'> = {
    txState: async (sid) => state(sid),
    txModeSet: async (sid, mode) => {
      of(sid).mode = mode;
      return state(sid);
    },
    txCommit: async (sid, confirm) => {
      const t = of(sid);
      if (!t.open) return state(sid);
      if (t.failed) throw new ApiError({ code: 'transaction_failed' });
      if (production(sid)) {
        if (!(await opts.access(sid)).open) throw new ApiError({ code: 'read_only' });
        const mode = readPrefs().safety.confirm_mode;
        if (t.dry && mode !== 'none' && confirm === null) throw new ApiError({ code: 'needs_confirmation' });
        if (t.dry && mode === 'typed' && confirm?.trim() !== opts.profile(sid)?.name.trim()) throw new ApiError({ code: 'name_mismatch' });
      }
      end(t);
      return state(sid);
    },
    txRollback: async (sid) => {
      end(of(sid));
      return state(sid);
    },
  };

  function holdApply(apply: Api['dataApply']): Api['dataApply'] {
    return async (sid, set, confirm) => {
      const t = of(sid);
      const out = await apply(sid, set, confirm);
      if (out.kind !== 'applied' || (!t.open && t.mode === 'auto')) return out;
      if (!t.open) begin(t, false);
      t.writes += out.rows ?? 0;
      t.rows += out.rows ?? 0;
      return { ...out, held: true };
    };
  }

  return { wrap, api, holdApply };
}
