import type { LockWait_Serialize as LockWait, Maintenance, ServerSession_Serialize as ServerSession } from './bindings';
import { duration, type Flavor, type Variant } from './tools';
import { getLocale } from '../paraglide/runtime';
import { m } from '../paraglide/messages';

export interface LockRow {
  session: ServerSession;
  depth: number;
  waiters: number;
  last: boolean;
  rails: boolean[];
  on?: LockWait;
}

export function lockTree(sessions: readonly ServerSession[], locks: readonly LockWait[] = []): LockRow[] {
  const byId = new Map(sessions.map((s) => [s.id, s]));
  const waitsOn = new Map<string, Set<string>>();
  const add = (waiting: string, blocking: string) => {
    if (waiting === blocking) return;
    const set = waitsOn.get(waiting) ?? new Set<string>();
    set.add(blocking);
    waitsOn.set(waiting, set);
  };
  for (const s of sessions) for (const b of s.blocked_by) add(s.id, b);
  for (const l of locks) add(l.waiting, l.blocking);
  const children = new Map<string, string[]>();
  for (const [waiting, blockers] of waitsOn) {
    for (const b of blockers) children.set(b, [...(children.get(b) ?? []), waiting]);
  }
  for (const list of children.values()) list.sort(byNumber);
  const sessionOf = (id: string): ServerSession =>
    byId.get(id) ?? { id, user: null, database: null, client: null, application: null, state: null, query: null, duration_ms: null, wait: null, blocked_by: [] };

  const heads = [...children.keys()].filter((id) => !waitsOn.has(id)).sort(byNumber);
  const placed = new Set<string>();
  for (const id of [...children.keys()].sort(byNumber)) {
    if (!heads.includes(id) && !reaches(id, heads, waitsOn)) heads.push(id);
  }

  const rows: LockRow[] = [];
  const walk = (id: string, depth: number, last: boolean, rails: boolean[], parent: string | null = null) => {
    if (placed.has(id)) return 0;
    placed.add(id);
    const on = parent === null ? undefined : locks.find((l) => l.waiting === id && l.blocking === parent && (l.object || l.mode));
    const row: LockRow = { session: sessionOf(id), depth, waiters: 0, last, rails, ...(on ? { on } : {}) };
    rows.push(row);
    const kids = (children.get(id) ?? []).filter((k) => !placed.has(k));
    kids.forEach((k, i) => {
      const end = i === kids.length - 1;
      row.waiters += 1 + walk(k, depth + 1, end, depth === 0 ? [] : [...rails, !last], id);
    });
    return row.waiters;
  };
  for (const h of heads) walk(h, 0, true, []);
  return rows;
}

function reaches(id: string, heads: readonly string[], waitsOn: Map<string, Set<string>>): boolean {
  const seen = new Set<string>();
  const stack = [id];
  while (stack.length) {
    const at = stack.pop() ?? '';
    if (seen.has(at)) continue;
    seen.add(at);
    for (const b of waitsOn.get(at) ?? []) {
      if (heads.includes(b)) return true;
      stack.push(b);
    }
  }
  return false;
}

function byNumber(a: string, b: string): number {
  const x = Number(a);
  const y = Number(b);
  return Number.isFinite(x) && Number.isFinite(y) ? x - y : a.localeCompare(b);
}

export function lockText(l: LockWait): string {
  const meta = l.mode?.startsWith('metadata ');
  const mode = meta ? (l.mode ?? '').slice('metadata '.length) : (l.mode ?? '');
  const what = meta ? m.mon_lock_metadata({ mode }) : mode;
  return l.object ? m.mon_lock_on({ what: what || m.mon_lock_plain(), object: l.object }) : what;
}

export const INTERVALS = [0, 2000, 5000, 10_000, 30_000, 60_000] as const;
export const DEFAULT_INTERVAL = 5000;

export const LONG_TX_MS = 60_000;

export function ageLevel(ms: number | null): 'ok' | 'warn' | 'bad' {
  if (ms === null) return 'ok';
  if (ms >= LONG_TX_MS * 5) return 'bad';
  return ms >= LONG_TX_MS ? 'warn' : 'ok';
}

export const MAINTENANCE_LABEL: Record<Maintenance, () => string> = {
  vacuum: m.mon_do_vacuum,
  analyze: m.mon_do_analyze,
  optimize: m.mon_do_optimize,
};

export function maintenanceOf(flavor: Variant, scope: 'table' | 'database'): Maintenance[] {
  const table = scope === 'table';
  switch (flavor) {
    case 'yugabyte':
      return ['analyze'];
    case 'tidb':
      return table ? ['analyze'] : [];
    case 'postgres':
    case 'timescale':
      return ['vacuum', 'analyze'];
    case 'cockroach':
      return table ? ['analyze'] : [];
    case 'mysql':
    case 'mariadb':
      return table ? ['optimize', 'analyze'] : [];
    case 'mssql':
      return ['analyze'];
    case 'sqlite':
      return table ? ['analyze'] : ['vacuum', 'analyze'];
    case 'duckdb':
      return table ? ['analyze'] : ['analyze', 'vacuum'];
    case 'clickhouse':
      return table ? ['optimize'] : [];
    case 'oracle':
      return ['analyze'];
    case 'firebird':
    case 'hana':
    case 'db2':
    case 'ansi':
      return [];
  }
}

export function maintenanceSql(flavor: Flavor, action: Maintenance, table: string | null): string {
  const on = (head: string) => (table ? `${head} ${table}` : head);
  if (action === 'optimize') return flavor === 'clickhouse' ? `${on('OPTIMIZE TABLE')} FINAL` : on('OPTIMIZE TABLE');
  if (action === 'vacuum') {
    if (flavor === 'postgres') return on('VACUUM (ANALYZE)');
    return flavor === 'duckdb' ? 'CHECKPOINT' : 'VACUUM';
  }
  if (flavor === 'mysql') return on('ANALYZE TABLE');
  if (flavor === 'mssql') return table ? `UPDATE STATISTICS ${table}` : 'EXEC sp_updatestats';
  if (flavor === 'oracle') return table ? `DBMS_STATS.GATHER_TABLE_STATS(${table})` : 'DBMS_STATS.GATHER_SCHEMA_STATS(USER)';
  return on('ANALYZE');
}

export function hasPlaceholders(sql: string, flavor: Flavor | null = null): boolean {
  const bare = sql.replace(/'(?:[^']|'')*'/g, "''");
  return /\$\d+\b|\?/.test(bare) || (flavor === 'mssql' && /(^|[^@\w])@[A-Za-z_]\w*/.test(bare));
}

export function explainSql(flavor: Flavor, sql: string): string {
  if (flavor === 'mssql') {
    const head = /^\s*\(\s*@/.exec(sql);
    if (!head) return sql;
    let depth = 0;
    for (let i = head[0].indexOf('('); i < sql.length; i++) {
      if (sql[i] === '(') depth++;
      else if (sql[i] === ')' && --depth === 0) return sql.slice(i + 1).trimStart();
    }
    return sql;
  }
  if (flavor === 'cockroach') {
    let n = 0;
    return sql
      .replace(/,\s*__more\w*__/g, '')
      .split(/('(?:[^']|'')*')/)
      .map((part, i) => (i % 2 ? part : part.replace(/(^|[\s,(=<>!+\-*/])_(?=$|[\s,);])/g, (_, before: string) => `${before}$${++n}`)))
      .join('');
  }
  return sql;
}

const EXPLAINABLE: readonly Flavor[] = ['postgres', 'cockroach', 'mysql', 'mssql', 'sqlite', 'duckdb', 'clickhouse', 'oracle', 'firebird', 'hana', 'db2'];

export const explainable = (flavor: Flavor) => EXPLAINABLE.includes(flavor);

export function percent(ratio: number | null): string {
  if (ratio === null || !Number.isFinite(ratio)) return '—';
  const tag = getLocale() === 'it' ? 'it-IT' : 'en-GB';
  return new Intl.NumberFormat(tag, { style: 'percent', minimumFractionDigits: 1, maximumFractionDigits: 1 }).format(Math.max(0, Math.min(1, ratio)));
}

export function took(ms: number | null): string {
  if (ms === null || !Number.isFinite(ms)) return '—';
  if (ms >= 10) return duration(ms);
  const tag = getLocale() === 'it' ? 'it-IT' : 'en-GB';
  return `${new Intl.NumberFormat(tag, { maximumFractionDigits: ms < 1 ? 2 : 1 }).format(ms)} ms`;
}

export const oneLine = (sql: string | null) => (sql ?? '').replace(/\s+/g, ' ').trim();
