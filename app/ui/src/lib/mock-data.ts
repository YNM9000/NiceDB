import { ApiError, type ApplyOutcome, type AuditEntry, type Browse, type ChangeSet, type ConnectionProfile, type Page, type RunEvent, type RunRequest, type WriteAccess } from './api';
import { gate } from './prodsafety';
import { readPrefs } from './mock-notes';
import type { Change_Deserialize as Change, Field_Deserialize as Field, Risk, RiskFound_Serialize as RiskFound } from './bindings';

type Cell = string | null;
type Gen = { columns: [string, string][]; total: number; row: (i: number) => Cell[] };

interface Store {
  rows: Map<string, Cell[]>;
  gone: Set<string>;
  added: Cell[][];
}

const UNLOCK_MS = 10 * 60 * 1000;
const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));

const quote = (v: Cell | undefined) => (v === null || v === undefined ? 'NULL' : `'${v.replaceAll("'", "''")}'`);
const ident = (s: string) => `"${s.replaceAll('"', '""')}"`;
const eq = (f: Field) => (f.value === null || f.value === undefined ? `${ident(f.column)} IS NULL` : `${ident(f.column)} = ${quote(f.value)}`);

function preview(table: string, c: Change): string {
  switch (c.kind) {
    case 'update':
      return `UPDATE ${ident('public')}.${ident(table)} SET ${c.set.map((f) => `${ident(f.column)} = ${quote(f.value)}`).join(', ')} WHERE ${[...c.key, ...c.original.filter((o) => !c.key.some((k) => k.column === o.column))].map(eq).join(' AND ')}`;
    case 'insert':
      return c.values.length
        ? `INSERT INTO ${ident('public')}.${ident(table)} (${c.values.map((f) => ident(f.column)).join(', ')}) VALUES (${c.values.map((f) => quote(f.value)).join(', ')})`
        : `INSERT INTO ${ident('public')}.${ident(table)} DEFAULT VALUES`;
    case 'delete':
      return `DELETE FROM ${ident('public')}.${ident(table)} WHERE ${[...c.key, ...c.original.filter((o) => !c.key.some((k) => k.column === o.column))].map(eq).join(' AND ')}`;
  }
}

function risk(sql: string): Risk | null {
  const s = sql.replace(/--.*$/gm, '').replace(/'[^']*'/g, "''").trim().toLowerCase();
  if (/^update\b/.test(s) && !/\bwhere\b/.test(s)) return 'update_without_where';
  if (/^delete\b/.test(s) && !/\bwhere\b/.test(s)) return 'delete_without_where';
  if (/^drop\b/.test(s)) return 'drop';
  if (/^truncate\b/.test(s)) return 'truncate';
  if (/^alter\b/.test(s)) return 'alter';
  return null;
}

const isRead = (sql: string) => /^\s*(select|with|show|explain|values)\b/i.test(sql) && !/\b(insert|update|delete)\b/i.test(sql);

function statements(text: string) {
  const out: { from: number; to: number; sql: string }[] = [];
  let start = 0;
  for (let i = 0; i <= text.length; i++) {
    if (i === text.length || text[i] === ';') {
      const raw = text.slice(start, i);
      const sql = raw.trim();
      if (sql) out.push({ from: start + raw.length - raw.trimStart().length, to: start + raw.length - raw.trimStart().length + sql.length, sql });
      start = i + 1;
    }
  }
  return out;
}

export function dataMock(tables: Record<string, Gen>, profile: (sessionId: string) => ConnectionProfile | undefined) {
  const stores = new Map<string, Store>();
  const unlocked = new Map<string, number>();
  const busy = new Set<string>();
  const unlockMs = () => Number(new URLSearchParams(typeof location === 'undefined' ? '' : location.search).get('unlock')) || UNLOCK_MS;
  const audit: AuditEntry[] = [];

  const store = (t: string) => {
    let s = stores.get(t);
    if (!s) stores.set(t, (s = { rows: new Map(), gone: new Set(), added: [] }));
    return s;
  };

  const rowsOf = (table: string, gen: Gen) => rows(table, gen);

  function rows(table: string, gen: Gen): Cell[][] {
    const s = store(table);
    const out: Cell[][] = [];
    for (let i = 1; i <= gen.total; i++) {
      const base = gen.row(i);
      const id = base[0] ?? String(i);
      if (s.gone.has(id)) continue;
      out.push(s.rows.get(id) ?? base);
    }
    return [...out, ...s.added];
  }

  function find(table: string, gen: Gen, key: Field[]): Cell[] | undefined {
    const col = gen.columns.findIndex(([n]) => n === key[0]?.column);
    const id = key[0]?.value ?? null;
    const s = store(table);
    const added = s.added.find((r) => r[col] === id);
    if (added) return added;
    const n = /^FT(\d+)$/.exec(id ?? '')?.[1] ? Number(id?.slice(2)) : Number(id);
    if (col !== 0 || !Number.isInteger(n) || n < 1 || n > gen.total) return undefined;
    const base = gen.row(n);
    return s.gone.has(base[0] ?? '') ? undefined : (s.rows.get(base[0] ?? '') ?? base);
  }

  const access = (sessionId: string): WriteAccess => {
    const guarded = profile(sessionId)?.environment === 'production';
    const until = unlocked.get(sessionId) ?? 0;
    const { unlock_mode: mode } = readPrefs().safety;
    const open = guarded && (mode === 'open' || until > Date.now());
    return { guarded, open, mode, open_ms: open && mode === 'timed' && until > Date.now() ? until - Date.now() : null };
  };

  function admit(sessionId: string, confirm: string | null | undefined) {
    const p = profile(sessionId);
    if (p?.environment !== 'production') return;
    const code = gate(readPrefs().safety.confirm_mode, access(sessionId).open, p.name, confirm);
    if (code) throw new ApiError({ code });
  }

  function log(sessionId: string, sql: string, rows: number | null, error: string | null) {
    audit.unshift({ id: audit.length + 1, connection_id: profile(sessionId)?.id ?? sessionId, database: sessionId.includes('@') ? sessionId.split('@')[1] ?? '' : 'gestionale', at: Date.now(), who: 'ada', sql, rows, error });
  }

  return {
    dataBrowse: async (_sessionId: string, b: Browse): Promise<Page> => {
      await sleep(60);
      const gen = tables[b.table.name];
      if (!gen) throw new ApiError({ code: 'not_found', detail: b.table.name });
      const cols = gen.columns.map(([n]) => n);
      let all = rows(b.table.name, gen);
      for (const q of b.quick) {
        const i = cols.indexOf(q.column);
        const v = (q.value ?? '').toLowerCase();
        all = all.filter((r) => {
          const c = r[i] ?? null;
          if (q.op === 'is_null') return c === null;
          if (q.op === 'not_null') return c !== null;
          if (c === null) return false;
          if (q.op === 'contains') return c.toLowerCase().includes(v);
          const [x, y] = Number.isFinite(Number(c)) && Number.isFinite(Number(v)) ? [Number(c), Number(v)] : [c, q.value ?? ''];
          return { eq: x === y, ne: x !== y, lt: x < y, le: x <= y, gt: x > y, ge: x >= y }[q.op];
        });
      }
      const where = b.filter?.trim();
      if (where) {
        const m = /^(\w+)\s*(=|>=|<=|>|<)\s*'?([^']*)'?$/.exec(where);
        if (!m) throw new ApiError({ code: 'driver', detail: { kind: 'query', message: `errore di sintassi in «${where}»`, code: '42601', detail: null, hint: null, offset: null } });
        const i = cols.indexOf(m[1] ?? '');
        if (i < 0) throw new ApiError({ code: 'driver', detail: { kind: 'query', message: `la colonna «${m[1]}» non esiste`, code: '42703', detail: null, hint: null, offset: null } });
        const v = m[3] ?? '';
        all = all.filter((r) => {
          const c = r[i] ?? '';
          const [x, y] = Number.isFinite(Number(c)) && Number.isFinite(Number(v)) ? [Number(c), Number(v)] : [c, v];
          return { '=': x === y, '>=': x >= y, '<=': x <= y, '>': x > y, '<': x < y }[m[2] as '='];
        });
      }
      if (b.order.length && all.length <= 200_000) {
        const keys = b.order.map((o) => ({ i: cols.indexOf(o.column), d: o.descending ? -1 : 1 }));
        all = [...all].sort((p, q) => {
          for (const { i, d } of keys) {
            const [x, y] = [p[i] ?? '', q[i] ?? ''];
            const c = Number.isFinite(Number(x)) && Number.isFinite(Number(y)) && x !== '' && y !== '' ? Number(x) - Number(y) : x.localeCompare(y, 'it');
            if (c) return c * d;
          }
          return 0;
        });
      }
      const byKey = b.order.every((o) => b.key.includes(o.column) && !o.descending);
      const after = byKey ? b.after?.[0] : undefined;
      const seen = after ? all.findIndex((r) => r[0] === after) : -1;
      const offset = seen >= 0 ? seen + 1 : (b.offset ?? 0);
      const slice = all.slice(offset, offset + b.limit + 1);
      const more = slice.length > b.limit;
      return {
        columns: gen.columns.map(([name]) => ({ name, type_name: 'text' })),
        rows: slice.slice(0, b.limit),
        has_more: more,
        next: more && byKey ? [slice[b.limit - 1]?.[0] ?? null] : null,
        total: (b.offset ?? 0) === 0 ? all.length : null,
        estimated: false,
        sql: `SELECT … FROM public.${b.table.name}`,
        elapsed_ms: 4,
      };
    },
    dataPreview: async (_s: string, set: ChangeSet) => ({ begin: 'BEGIN', statements: set.changes.map((c) => preview(set.table.name, c)), commit: 'COMMIT' }),
    dataEstimate: async (_s: string, set: ChangeSet): Promise<(number | null)[]> => {
      await sleep(40);
      const gen = tables[set.table.name];
      if (!gen) return set.changes.map(() => null);
      const cols = gen.columns.map(([n]) => n);
      return set.changes.map((c) => {
        if (c.kind === 'insert') return null;
        const row = find(set.table.name, gen, c.key);
        return row && !c.original.some((o) => (row[cols.indexOf(o.column)] ?? null) !== (o.value ?? null)) ? 1 : 0;
      });
    },
    dataApply: async (sessionId: string, set: ChangeSet, confirm: string | null): Promise<ApplyOutcome> => {
      if (set.changes.some((c) => c.kind !== 'insert' && c.key.length === 0)) throw new ApiError({ code: 'no_key' });
      admit(sessionId, confirm);
      await sleep(80);
      const gen = tables[set.table.name];
      if (!gen) throw new ApiError({ code: 'not_found', detail: set.table.name });
      const cols = gen.columns.map(([n]) => n);
      const s = store(set.table.name);
      const before = { rows: new Map(s.rows), gone: new Set(s.gone), added: s.added.map((r) => [...r]) };
      const script = set.changes.map((c) => preview(set.table.name, c));
      for (const [index, c] of set.changes.entries()) {
        if (c.kind === 'insert') {
          const row: Cell[] = cols.map((_n, i) => (i === 0 ? String(gen.total + s.added.length + 1) : null));
          for (const f of c.values) row[cols.indexOf(f.column)] = f.value ?? null;
          s.added.push(row);
          continue;
        }
        const row = find(set.table.name, gen, c.key);
        const stale = c.original.some((o) => (row?.[cols.indexOf(o.column)] ?? null) !== (o.value ?? null));
        if (!row || stale) {
          Object.assign(s, before);
          log(sessionId, script.join(';\n'), null, `conflict at change ${index + 1}`);
          return { kind: 'conflict', index };
        }
        const id = row[0] ?? '';
        if (c.kind === 'delete') {
          if (s.added.includes(row)) s.added = s.added.filter((r) => r !== row);
          else s.gone.add(id);
        } else {
          const next = [...row];
          for (const f of c.set) next[cols.indexOf(f.column)] = f.value ?? null;
          const at = s.added.indexOf(row);
          if (at >= 0) s.added[at] = next;
          else s.rows.set(id, next);
        }
      }
      log(sessionId, script.join(';\n'), set.changes.length, null);
      return { kind: 'applied', rows: set.changes.length, held: false };
    },
    writeAccess: async (sessionId: string) => access(sessionId),
    writesUnlock: async (sessionId: string) => {
      if (access(sessionId).guarded) unlocked.set(sessionId, Date.now() + (readPrefs().safety.unlock_mode === 'timed' ? unlockMs() : 10 * 365 * 86_400_000));
      return access(sessionId);
    },
    writesLock: async (sessionId: string) => {
      if (busy.has(sessionId)) throw new ApiError({ code: 'busy' });
      if (readPrefs().safety.unlock_mode !== 'open') unlocked.delete(sessionId);
      return access(sessionId);
    },
    auditList: async (connectionId: string, limit: number) => audit.filter((a) => a.connection_id === connectionId).slice(0, limit),
    guardRun: async (req: RunRequest, emit: (e: RunEvent) => void, run: (req: RunRequest, emit: (e: RunEvent) => void) => Promise<void>) => {
      const all = statements(req.text);
      const risks: RiskFound[] = all.flatMap((st, index) => {
        const r = risk(st.sql);
        const table = /^\s*(?:update|delete\s+from)\s+(?:public\.)?(\w+)/i.exec(st.sql)?.[1] ?? '';
        const gen = tables[table];
        const rows = (r === 'update_without_where' || r === 'delete_without_where') && gen ? rowsOf(table, gen).length : null;
        return r ? [{ index, from: st.from, to: st.to, sql: st.sql, risk: r, rows }] : [];
      });
      if (risks.length && !req.confirm?.risks && !(profile(req.session_id)?.environment === 'production' && readPrefs().safety.confirm_mode === 'none')) throw new ApiError({ code: 'risky', detail: risks });
      const writes = all.filter((st) => !isRead(st.sql));
      if (writes.length) admit(req.session_id, req.confirm?.name);
      if (!writes.length) return run(req, emit);
      const started = performance.now();
      const wasOpen = access(req.session_id).open_ms !== null;
      busy.add(req.session_id);
      for (const [index, st] of all.entries()) {
        emit({ type: 'statement', index, from: st.from, to: st.to, sql: st.sql, unsortable: null });
        const wait = Number(/pg_sleep\((\d+(?:\.\d+)?)\)/i.exec(st.sql)?.[1] ?? 0);
        if (wait) await sleep(wait * 1000);
        const command = st.sql.split(/\s+/)[0]?.toUpperCase() ?? null;
        const affected = isRead(st.sql) ? null : 3;
        emit({ type: 'done', index, elapsed_ms: 3, row_count: 0, rows_affected: affected, command, truncated: false, row_limit: null });
        if (affected !== null) log(req.session_id, st.sql, affected, null);
      }
      busy.delete(req.session_id);
      if (wasOpen && access(req.session_id).open_ms === null) {
        unlocked.delete(req.session_id);
        emit({ type: 'relocked' });
      }
      emit({ type: 'finished', elapsed_ms: Math.round(performance.now() - started), cancelled: false });
    },
  };
}
