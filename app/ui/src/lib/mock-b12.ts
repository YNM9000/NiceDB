import { ApiError, type ConnectionProfile, type Monitor, type SlowQueries, type WriteAccess } from './api';
import type { MaintenanceDone, MaintenanceRequest_Deserialize as MaintenanceRequest } from './bindings';
import { readPrefs } from './mock-notes';
import { gate } from './prodsafety';

const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));
const MB = 1024 * 1024;

const TRANSACTIONS: Monitor['transactions'] = [
  { id: '18297', user: 'mrossi', database: 'gestionale', age_ms: 134_000, state: 'idle in transaction', query: 'SELECT * FROM fatture WHERE cliente_id = 812 FOR UPDATE' },
  { id: '18402', user: 'backup', database: 'gestionale', age_ms: 62_000, state: 'active', query: 'COPY public.righe_ordine (id, ordine_id, articolo_id, quantita, prezzo, sconto) TO stdout;' },
  { id: '18410', user: 'report', database: 'gestionale', age_ms: 48_300, state: 'active', query: "SELECT c.ragione_sociale, sum(r.quantita * r.prezzo) AS venduto FROM righe_ordine r JOIN ordini o ON o.id = r.ordine_id JOIN clienti c ON c.id = o.cliente_id WHERE o.data >= date_trunc('year', now()) GROUP BY 1 ORDER BY 2 DESC" },
  { id: '18342', user: 'erp_web', database: 'gestionale', age_ms: 4200, state: 'active', query: 'UPDATE fatture SET pagata = true, data_pagamento = $1 WHERE numero = $2' },
];

const PG_SLOW: SlowQueries['queries'] = [
  { text: "SELECT c.ragione_sociale, sum(r.quantita * r.prezzo) AS venduto FROM righe_ordine r JOIN ordini o ON o.id = r.ordine_id JOIN clienti c ON c.id = o.cliente_id WHERE o.data >= date_trunc($1, now()) GROUP BY 1 ORDER BY 2 DESC", sample: null, calls: 212, total_ms: 9_840_000, mean_ms: 46_415, rows: 98_000 },
  { text: 'SELECT numero, cliente_id, totale FROM fatture WHERE NOT pagata AND scadenza < current_date ORDER BY scadenza', sample: null, calls: 18_400, total_ms: 1_204_000, mean_ms: 65.4, rows: 2_310_000 },
  { text: 'UPDATE fatture SET pagata = true, data_pagamento = $1 WHERE numero = $2', sample: null, calls: 9120, total_ms: 402_000, mean_ms: 44.1, rows: 9120 },
  { text: 'SELECT id, ragione_sociale FROM clienti WHERE partita_iva = $1', sample: null, calls: 412_000, total_ms: 188_000, mean_ms: 0.46, rows: 411_200 },
  { text: 'INSERT INTO righe_ordine (ordine_id, articolo_id, quantita, prezzo, sconto) VALUES ($1, $2, $3, $4, $5)', sample: null, calls: 96_300, total_ms: 91_200, mean_ms: 0.95, rows: 96_300 },
];

const MY_SLOW: SlowQueries['queries'] = PG_SLOW.map((q) => ({
  ...q,
  text: q.text.replace(/\$\d/g, '?'),
  sample: q.text.includes('$') ? q.text.replace('$1', "'2026-01-01'").replace('$2', "'FT-2026-0412'").replace(/\$\d/g, '1') : q.text,
}));

const gaps = (...parts: Monitor['gaps'][number]['part'][]): Monitor['gaps'] => parts.map((part) => ({ part, reason: { kind: 'file_database' } }));

function monitorOf(driver: string, database: string): Monitor {
  const transactions = structuredClone(TRANSACTIONS);
  switch (driver) {
    case 'sqlite':
    case 'libsql':
      return { connections: null, transactions: [], databases: [{ name: 'main', bytes: 9.1 * MB }], cache_hit: null, gaps: gaps('connections', 'transactions', 'cache_hit') };
    case 'duckdb':
      return { connections: null, transactions: [], databases: [{ name: 'gestionale', bytes: 14.2 * MB }], cache_hit: null, gaps: gaps('connections', 'transactions', 'cache_hit') };
    case 'cockroach':
      return { connections: { total: 9, active: 2, idle_in_transaction: null, waiting: null, max: null }, transactions, databases: [{ name: database, bytes: 286 * MB }], cache_hit: null, gaps: [{ part: 'cache_hit', reason: { kind: 'not_exposed' } }] };
    case 'mysql':
    case 'mariadb':
      return {
        connections: { total: 31, active: 3, idle_in_transaction: null, waiting: 1, max: 151 },
        transactions,
        databases: [
          { name: 'gestionale', bytes: 298 * MB },
          { name: 'gestionale_archivio', bytes: 1210 * MB },
        ].sort((a, b) => b.bytes - a.bytes),
        cache_hit: 0.9962,
        gaps: [],
      };
    default:
      return {
        connections: { total: 23, active: 4, idle_in_transaction: 1, waiting: 1, max: 100 },
        transactions,
        databases: [
          { name: 'gestionale_archivio', bytes: 1480 * MB },
          { name: 'gestionale', bytes: 312 * MB },
          { name: 'postgres', bytes: 7.6 * MB },
        ],
        cache_hit: 0.9871,
        gaps: [],
      };
  }
}

function slowOf(driver: string, off: boolean): SlowQueries {
  switch (driver) {
    case 'sqlite':
    case 'libsql':
      return { source: null, gap: { kind: 'file_database' }, queries: [] };
    case 'duckdb':
      return { source: null, gap: { kind: 'not_exposed' }, queries: [] };
    case 'mariadb':
      return { source: null, gap: { kind: 'feature_off', name: 'performance_schema' }, queries: [] };
    case 'mysql':
      return { source: 'performance_schema', gap: null, queries: structuredClone(MY_SLOW) };
    case 'sqlserver':
      return { source: 'query_store', gap: null, queries: structuredClone(MY_SLOW).map((q) => ({ ...q, text: q.sample ?? q.text })) };
    case 'cockroach':
      return { source: 'statement_statistics', gap: null, queries: structuredClone(PG_SLOW) };
    default:
      return off ? { source: null, gap: { kind: 'extension_missing', name: 'pg_stat_statements' }, queries: [] } : { source: 'pg_stat_statements', gap: null, queries: structuredClone(PG_SLOW) };
  }
}

export interface MonitorMockOptions {
  profile: (sessionId: string) => ConnectionProfile | undefined;
  access: (sessionId: string) => Promise<WriteAccess>;
}

export function monitorMock({ profile, access }: MonitorMockOptions) {
  const driverOf = (sid: string) => {
    const p = profile(sid.split('@')[0] ?? sid);
    return p?.target.driver ?? 'postgres';
  };
  const databaseOf = (sid: string) => sid.split('@')[1] ?? 'gestionale';
  return {
    serverMonitor: async (sid: string): Promise<Monitor> => {
      await sleep(50);
      return monitorOf(driverOf(sid), databaseOf(sid));
    },
    serverSlowQueries: async (sid: string): Promise<SlowQueries> => {
      await sleep(80);
      return slowOf(driverOf(sid), new URLSearchParams(location.search).get('pgss') === 'off');
    },
    serverMaintain: async (req: MaintenanceRequest): Promise<MaintenanceDone> => {
      const p = profile(req.session_id.split('@')[0] ?? req.session_id);
      if (p?.environment === 'production') {
        const code = gate(readPrefs().safety.confirm_mode, (await access(req.session_id)).open, p.name, req.confirm ?? null);
        if (code) throw new ApiError({ code });
      }
      await sleep(400);
      const t = req.table ? ` ${req.table.namespace}.${req.table.name}` : '';
      const sql = { vacuum: `VACUUM (ANALYZE)${t}`, analyze: `ANALYZE${t}`, optimize: `OPTIMIZE TABLE${t}` }[req.action];
      return { sql, messages: req.action === 'optimize' ? ['status: OK'] : [], elapsed_ms: 400 };
    },
  };
}
