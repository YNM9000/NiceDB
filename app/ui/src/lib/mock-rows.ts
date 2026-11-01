import { ApiError, type ConnectionProfile, type RowCompared, type RowCompareEvent, type RowCompareRequest, type RowProbe, type RowSource } from './api';
import type { Column_Serialize as Column, ColumnPlan_Serialize as ColumnPlan, Dialect, RowDiff_Serialize as RowDiff } from './bindings';
import { quoteName } from './sqlname';

const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));

type Cells = (string | null)[];

const COLUMNS: Column[] = [
  { name: 'id', type_name: 'int4' },
  { name: 'ragione_sociale', type_name: 'text' },
  { name: 'citta', type_name: 'varchar' },
  { name: 'provincia', type_name: 'bpchar' },
  { name: 'fido', type_name: 'numeric' },
];

const NAMES = [
  ['Officine Mangili S.r.l.', 'Bergamo', 'BG', '15000.00'],
  ['Termoidraulica Pezzotta S.n.c.', 'Lumezzane', 'BS', '8000.00'],
  ['Serramenti Facchinetti S.p.A.', 'Seriate', 'BG', '22000.00'],
  ['Carpenteria Bonomi S.a.s.', 'Rovato', 'BS', null],
  ['Edilnord Locatelli S.r.l.', 'Treviglio', 'BG', '12000.00'],
  ['Falegnameria Gatti S.n.c.', 'Chiari', 'BS', '6500.00'],
  ['Autotrasporti Ghisleni S.r.l.', 'Dalmine', 'BG', '30000.00'],
  ['Tornerie Zanardelli S.p.A.', 'Gardone Val Trompia', 'BS', '45000.00'],
  ['Impianti Rota S.r.l.', 'Ponte San Pietro', 'BG', '9000.00'],
  ['Vetreria Belotti S.n.c.', 'Palazzolo sull’Oglio', 'BS', '4000.00'],
  ['Stampaggi Cortinovis S.r.l.', 'Curno', 'BG', '18000.00'],
  ['Minuterie Pedretti S.a.s.', 'Ospitaletto', 'BS', '7500.00'],
] as const;

const base = (): Cells[] => Array.from({ length: 120 }, (_, i) => {
  const n = NAMES[i % NAMES.length] ?? NAMES[0];
  return [String(2401 + i), i < NAMES.length ? n[0] : `${n[0].split(' ')[0]} ${2401 + i}`, n[1], n[2], n[3]];
});

function dataset(production: boolean): Cells[] {
  const rows = base();
  if (!production) {
    rows[2] = ['2403', 'Serramenti Facchinetti S.p.A.', 'Bergamo', 'BG', '25000.00'];
    rows[5] = ['2406', 'Falegnameria Gatti S.n.c.', 'Chiari', 'BS', null];
    rows[9] = ['2410', 'Vetreria Belotti & Figli S.n.c.', 'Palazzolo sull’Oglio', 'BS', '4000.00'];
    rows.splice(7, 1);
    rows.splice(40, 1);
    rows.push(['2521', 'Fonderia Lazzaroni S.p.A.', 'Sarnico', 'BG', '60000.00'], ['2522', 'Elettromeccanica Poli S.r.l.', 'Iseo', 'BS', null], ['2523', 'Laminati Togni S.r.l.', 'Villongo', 'BG', '11000.00']);
  }
  return rows;
}

const kept = new Map<string, { dialect: Dialect; plan: ColumnPlan; key: number[]; rows: RowDiff[] }>();

export function rowsMock(profile: (sid: string) => ConnectionProfile | undefined, jobs: Map<string, () => void>) {
  const production = (s: RowSource) => profile(s.session_id)?.environment === 'production';
  const dialect = (s: RowSource): Dialect => {
    const d = profile(s.session_id)?.target.driver;
    return d === 'mysql' ? 'mysql' : d === 'sqlserver' ? 'mssql' : d === 'sqlite' || d === 'libsql' ? 'sqlite' : d === 'duckdb' ? 'duckdb' : 'postgres';
  };
  const readable = (s: RowSource) => {
    if (s.kind === 'query' && !/^\s*(select|with|values|table)\b/i.test(s.sql)) throw new ApiError({ code: 'tool', detail: { kind: 'not_read' } });
  };

  return {
    rowProbe: async (source: RowSource): Promise<RowProbe> => {
      readable(source);
      await sleep(60);
      return { columns: COLUMNS, dialect: dialect(source) };
    },
    rowCompare: async (req: RowCompareRequest, onEvent: (e: RowCompareEvent) => void): Promise<RowCompared> => {
      readable(req.a);
      readable(req.b);
      const plan: ColumnPlan = { common: COLUMNS.map((c) => ({ name: c.name, b_name: c.name, a_type: c.type_name, b_type: c.type_name, type_differs: false })), only_a: [], only_b: [] };
      const key = req.key.map((k) => COLUMNS.findIndex((c) => c.name === k));
      if (!key.length) throw new ApiError({ code: 'tool', detail: { kind: 'empty' } });
      const missing = req.key.find((_, i) => (key[i] ?? -1) < 0);
      if (missing) throw new ApiError({ code: 'tool', detail: { kind: 'key_missing', column: missing } });
      let cancelled = false;
      jobs.set(req.job_id, () => (cancelled = true));
      const started = Date.now();
      const a = dataset(production(req.a));
      const b = dataset(production(req.b));
      const id = (r: Cells) => key.map((k) => r[k] ?? '\u0001').join('\u0000');
      const byA = new Map(a.map((r) => [id(r), r]));
      const seen = new Set<string>();
      const rows: RowDiff[] = [];
      const counts = { added: 0, removed: 0, changed: 0, same: 0 };
      onEvent({ kind: 'plan', plan });
      for (const r of b) {
        const k = id(r);
        const other = byA.get(k);
        if (!other) continue;
        seen.add(k);
        const changes = r.flatMap((v, c) => (v === other[c] ? [] : [{ column: c, was: other[c] ?? null }]));
        rows.push({ state: changes.length ? 'changed' : 'same', cells: r, changes });
        counts[changes.length ? 'changed' : 'same']++;
      }
      for (const r of a) if (!seen.has(id(r))) (rows.push({ state: 'removed', cells: r, changes: [] }), counts.removed++);
      for (const r of b) if (!byA.has(id(r))) (rows.push({ state: 'added', cells: r, changes: [] }), counts.added++);
      const steps = 4;
      for (let i = 0; i < steps && !cancelled; i++) {
        await sleep(90);
        const part = rows.slice(Math.floor((rows.length * i) / steps), Math.floor((rows.length * (i + 1)) / steps));
        onEvent({ kind: 'rows', rows: part });
        onEvent({ kind: 'progress', a_rows: Math.round((a.length * (i + 1)) / steps), b_rows: Math.round((b.length * (i + 1)) / steps), waiting: 0 });
      }
      jobs.delete(req.job_id);
      if (!cancelled) kept.set(req.job_id, { dialect: dialect(req.a), plan, key, rows: rows.filter((r) => r.state !== 'same') });
      return { plan, counts, a_rows: a.length, b_rows: b.length, same_cut: false, cancelled, elapsed_ms: Date.now() - started, a_dialect: dialect(req.a), b_dialect: dialect(req.b) };
    },
    rowScript: async (compareId: string, namespace: string, name: string): Promise<string[]> => {
      const k = kept.get(compareId);
      if (!k) throw new ApiError({ code: 'not_found', detail: 'comparison' });
      const q = (n: string) => quoteName(k.dialect, n);
      const table = namespace ? `${q(namespace)}.${q(name)}` : q(name);
      const lit = (v: string | null, c: number) => (v === null ? 'NULL' : k.plan.common[c]?.a_type?.startsWith('int') || k.plan.common[c]?.a_type === 'numeric' ? v : `'${v.replaceAll("'", "''")}'`);
      const where = (r: RowDiff) => k.key.map((c) => `${q(k.plan.common[c]?.name ?? '')} = ${lit(r.cells[c] ?? null, c)}`).join(' AND ');
      const of = (s: RowDiff['state']) => k.rows.filter((r) => r.state === s);
      const out: string[] = [];
      const removed = of('removed');
      if (removed.length) out.push(`DELETE FROM ${table} WHERE ${q(k.plan.common[k.key[0] ?? 0]?.name ?? '')} IN (${removed.map((r) => lit(r.cells[k.key[0] ?? 0] ?? null, k.key[0] ?? 0)).join(', ')})`);
      for (const r of of('changed')) out.push(`UPDATE ${table} SET ${r.changes.map((c) => `${q(k.plan.common[c.column]?.name ?? '')} = ${lit(r.cells[c.column] ?? null, c.column)}`).join(', ')} WHERE ${where(r)}`);
      const added = of('added');
      if (added.length) out.push(`INSERT INTO ${table} (${k.plan.common.map((c) => q(c.name)).join(', ')}) VALUES\n  ${added.map((r) => `(${r.cells.map(lit).join(', ')})`).join(',\n  ')}`);
      return out;
    },
    rowForget: async (compareId: string) => {
      kept.delete(compareId);
    },
  };
}
