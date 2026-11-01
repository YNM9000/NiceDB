import { ApiError, type Activity, type Comparison, type ConnectionProfile, type DataImportSample, type Principals, type Progress, type Sizes, type ToolCheck, type WriteAccess } from './api';
import type { DiffItem_Serialize as DiffItem, Grant_Serialize as Grant, Unit } from './bindings';
import { readPrefs } from './mock-notes';
import { rowsMock } from './mock-rows';
import { gate } from './prodsafety';
import { SHOWCASE_SESSIONS } from './mock-showcase';

const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));
const MB = 1024 * 1024;

const SESSIONS: Activity['sessions'] = typeof location !== 'undefined' && new URLSearchParams(location.search).has('showcase') ? SHOWCASE_SESSIONS : [
  { id: '18342', user: 'erp_web', database: 'gestionale', client: '10.0.4.21', application: 'erp-web', state: 'active', query: 'UPDATE fatture SET pagata = true, data_pagamento = $1 WHERE numero = $2', duration_ms: 4200, wait: 'Lock: transactionid', blocked_by: ['18297'] },
  { id: '18297', user: 'mrossi', database: 'gestionale', client: '10.0.4.57', application: 'psql', state: 'idle in transaction', query: 'SELECT * FROM fatture WHERE cliente_id = 812 FOR UPDATE', duration_ms: 134_000, wait: 'Client: ClientRead', blocked_by: [] },
  { id: '18410', user: 'report', database: 'gestionale', client: '10.0.5.3', application: 'Metabase', state: 'active', query: 'SELECT c.ragione_sociale, sum(r.quantita * r.prezzo) AS venduto FROM righe_ordine r JOIN ordini o ON o.id = r.ordine_id JOIN clienti c ON c.id = o.cliente_id WHERE o.data >= date_trunc(\'year\', now()) GROUP BY 1 ORDER BY 2 DESC', duration_ms: 48_300, wait: 'IO: DataFileRead', blocked_by: [] },
  { id: '18402', user: 'backup', database: 'gestionale', client: '10.0.9.2', application: 'pg_dump', state: 'active', query: 'COPY public.righe_ordine (id, ordine_id, articolo_id, quantita, prezzo, sconto) TO stdout;', duration_ms: 62_000, wait: null, blocked_by: [] },
  { id: '18388', user: 'erp_web', database: 'gestionale', client: '10.0.4.21', application: 'erp-web', state: 'idle', query: 'COMMIT', duration_ms: null, wait: 'Client: ClientRead', blocked_by: [] },
  { id: '18355', user: 'erp_web', database: 'gestionale', client: '10.0.4.22', application: 'erp-web', state: 'idle', query: 'SELECT id, ragione_sociale FROM clienti WHERE partita_iva = $1', duration_ms: null, wait: 'Client: ClientRead', blocked_by: [] },
];

const SIZES: Sizes = {
  usage_known: true,
  tables: [
    { namespace: 'public', name: 'righe_ordine', rows: 1_200_000, data_bytes: 142 * MB, index_bytes: 61 * MB, total_bytes: 203 * MB },
    { namespace: 'public', name: 'ordini', rows: 120_000, data_bytes: 18.4 * MB, index_bytes: 7.1 * MB, total_bytes: 25.5 * MB },
    { namespace: 'public', name: 'fatture', rows: 90_000, data_bytes: 16.2 * MB, index_bytes: 6.3 * MB, total_bytes: 22.5 * MB },
    { namespace: 'public', name: 'articoli', rows: 4000, data_bytes: 1.1 * MB, index_bytes: 0.4 * MB, total_bytes: 1.5 * MB },
    { namespace: 'public', name: 'clienti', rows: 2400, data_bytes: 0.62 * MB, index_bytes: 0.21 * MB, total_bytes: 0.83 * MB },
    { namespace: 'public', name: 'listini', rows: 180, data_bytes: 48 * 1024, index_bytes: 16 * 1024, total_bytes: 64 * 1024 },
  ],
  indexes: [
    { namespace: 'public', table: 'righe_ordine', name: 'righe_ordine_pkey', bytes: 26 * MB, scans: 4_812_300, unique: true, unused: false },
    { namespace: 'public', table: 'righe_ordine', name: 'righe_ordine_ordine_idx', bytes: 11 * MB, scans: 912_004, unique: false, unused: false },
    { namespace: 'public', table: 'righe_ordine', name: 'righe_ordine_sconto_idx', bytes: 24 * MB, scans: 0, unique: false, unused: true },
    { namespace: 'public', table: 'ordini', name: 'ordini_pkey', bytes: 2.6 * MB, scans: 1_204_551, unique: true, unused: false },
    { namespace: 'public', table: 'ordini', name: 'ordini_cliente_idx', bytes: 2.4 * MB, scans: 88_120, unique: false, unused: false },
    { namespace: 'public', table: 'fatture', name: 'fatture_pkey', bytes: 2.1 * MB, scans: 640_022, unique: true, unused: false },
    { namespace: 'public', table: 'fatture', name: 'fatture_note_idx', bytes: 3.9 * MB, scans: 0, unique: false, unused: true },
    { namespace: 'public', table: 'clienti', name: 'clienti_partita_iva_key', bytes: 0.1 * MB, scans: 0, unique: true, unused: false },
  ],
};

const g = (grantee: string, kind: string, schema: string | null, object: string | null, privilege: string, grantable = false): Grant => ({ grantee, kind, schema, object, privilege, grantable });

const PRINCIPALS: Principals = {
  partial: false,
  quotas: [],
  principals: [
    { name: 'analisti', host: null, attributes: [], member_of: [] },
    { name: 'backup', host: null, attributes: ['login'], member_of: [] },
    { name: 'erp_web', host: null, attributes: ['login'], member_of: [] },
    { name: 'lettura', host: null, attributes: ['login'], member_of: ['analisti'] },
    { name: 'mrossi', host: null, attributes: ['login', 'create_db'], member_of: ['analisti'] },
    { name: 'nicedb', host: null, attributes: ['login', 'superuser', 'create_db', 'create_role'], member_of: [] },
    { name: 'report', host: null, attributes: ['login', 'locked'], member_of: ['lettura'] },
  ],
  grants: [
    g('PUBLIC', 'database', null, 'gestionale', 'CONNECT'),
    g('analisti', 'schema', 'public', null, 'USAGE'),
    g('erp_web', 'schema', 'public', null, 'USAGE'),
    ...['clienti', 'fatture', 'ordini', 'righe_ordine'].flatMap((t) => ['SELECT', 'INSERT', 'UPDATE', 'DELETE'].map((p) => g('erp_web', 'table', 'public', t, p))),
    ...['clienti', 'fatture', 'ordini', 'righe_ordine', 'articoli'].map((t) => g('lettura', 'table', 'public', t, 'SELECT')),
    g('lettura', 'view', 'public', 'v_scadenzario', 'SELECT'),
    g('backup', 'table', 'public', 'righe_ordine', 'SELECT'),
    ...['clienti', 'fatture', 'ordini', 'righe_ordine', 'articoli'].map((t) => g('nicedb', 'table', 'public', t, 'OWNER', true)),
  ],
};

const d = (kind: DiffItem['kind'], name: string, state: DiffItem['state'], source: string | null, target: string | null, table: string | null = null, notes: DiffItem['notes'] = []): DiffItem => ({ kind, name, state, source, target, table, notes });

const COMPARISON: Comparison = {
  source_dialect: 'postgres',
  target_dialect: 'postgres',
  items: [
    d('table', 'articoli', 'same', '9 col.', '9 col.'),
    d('table', 'clienti', 'changed', '8 col.', '7 col.'),
    d('column', 'citta', 'changed', 'varchar(80) NOT NULL', 'text', 'clienti'),
    d('column', 'pec', 'added', 'text', null, 'clienti'),
    d('index', 'clienti_citta_idx', 'added', '(citta)', null, 'clienti'),
    d('table', 'fatture', 'changed', '9 col.', '9 col.'),
    d('foreign_key', 'fatture_cliente_fk', 'changed', '(cliente_id) → clienti(id) ON DELETE CASCADE', '(cliente_id) → clienti(id) ON DELETE RESTRICT', 'fatture'),
    d('index', 'fatture_note_idx', 'removed', null, '(note)', 'fatture'),
    d('table', 'ordini', 'same', '7 col.', '7 col.'),
    d('table', 'righe_ordine', 'same', '6 col.', '6 col.'),
    d('table', 'solleciti', 'added', '5 col.', null),
    d('table', 'tmp_import_2023', 'removed', null, '4 col.', null, ['destructive']),
    d('view', 'v_scadenzario', 'changed', 'SELECT numero, scadenza, totale FROM fatture WHERE NOT pagata', 'SELECT numero, scadenza FROM fatture WHERE NOT pagata'),
    d('function', 'totale_cliente(cid integer)', 'same', null, null),
  ],
  statements: [
    'ALTER TABLE "public"."fatture" DROP CONSTRAINT "fatture_cliente_fk"',
    'DROP INDEX "public"."fatture_note_idx"',
    'CREATE TABLE "public"."solleciti" (\n  "id" serial NOT NULL,\n  "fattura" text NOT NULL,\n  "inviato_il" date NOT NULL,\n  "canale" text DEFAULT \'pec\'::text,\n  "esito" text,\n  PRIMARY KEY ("id")\n)',
    'ALTER TABLE "public"."clienti" ALTER COLUMN "citta" TYPE varchar(80) USING "citta"::varchar(80)',
    'ALTER TABLE "public"."clienti" ALTER COLUMN "citta" SET NOT NULL',
    'ALTER TABLE "public"."clienti" ADD COLUMN "pec" text',
    'CREATE INDEX "clienti_citta_idx" ON "public"."clienti" ("citta")',
    'DROP TABLE "public"."tmp_import_2023"',
    'ALTER TABLE "public"."fatture" ADD CONSTRAINT "fatture_cliente_fk" FOREIGN KEY ("cliente_id") REFERENCES "public"."clienti" ("id") ON DELETE CASCADE',
    'CREATE OR REPLACE VIEW "public"."v_scadenzario" AS\nSELECT numero, scadenza, totale FROM fatture WHERE NOT pagata',
  ],
};

const SAMPLE: DataImportSample = {
  sheets: [],
  more: true,
  columns: [
    { name: 'codice', inferred: 'integer', type_name: 'integer', nullable: false },
    { name: 'Ragione sociale', inferred: 'text', type_name: 'text', nullable: false },
    { name: 'Partita IVA', inferred: 'text', type_name: 'text', nullable: false },
    { name: 'Città', inferred: 'text', type_name: 'text', nullable: true },
    { name: 'provincia', inferred: 'text', type_name: 'text', nullable: true },
    { name: 'fido', inferred: 'decimal', type_name: 'numeric', nullable: true },
    { name: 'cliente dal', inferred: 'date', type_name: 'date', nullable: true },
  ],
  rows: [
    ['2401', 'Officine Mangili S.r.l.', '03981240165', 'Bergamo', 'BG', '15000.00', '2026-01-12'],
    ['2402', 'Termoidraulica Pezzotta S.n.c.', '02284110984', 'Lumezzane', 'BS', '8000.00', '2026-01-19'],
    ['2403', 'Serramenti Facchinetti S.p.A.', '01877320163', 'Seriate', 'BG', '22000.00', '2026-02-02'],
    ['2404', 'Carpenteria Bonomi S.a.s.', '03122450988', 'Rovato', 'BS', '', '2026-02-10'],
    ['2405', 'Edilnord Locatelli S.r.l.', '04211870161', 'Treviglio', 'BG', '12000.00', '2026-02-24'],
    ['2406', 'Falegnameria Gatti S.n.c.', '02955630987', 'Chiari', 'BS', '6500.00', '2026-03-03'],
  ],
};

export interface ToolsMockOptions {
  profile: (sessionId: string) => ConnectionProfile | undefined;
  access: (sessionId: string) => Promise<WriteAccess>;
}

export function toolsMock({ profile, access }: ToolsMockOptions) {
  const jobs = new Map<string, () => void>();
  const driver = (sid: string) => profile(sid)?.target.driver;
  const sqlite = (sid: string) => driver(sid) === 'sqlite' || driver(sid) === 'libsql';
  const duckdb = (sid: string) => driver(sid) === 'duckdb';
  const local = (sid: string) => sqlite(sid) || duckdb(sid);
  const unsupported = () => new ApiError({ code: 'tool', detail: { kind: 'unsupported' } });

  async function admit(sid: string, confirm: string | null | undefined) {
    const p = profile(sid);
    if (p?.environment !== 'production') return;
    const code = gate(readPrefs().safety.confirm_mode, (await access(sid)).open, p.name, confirm);
    if (code) throw new ApiError({ code });
  }

  async function tick(id: string, total: number, unit: Unit, onProgress: (p: Progress) => void, ms = 1600) {
    let cancelled = false;
    jobs.set(id, () => (cancelled = true));
    const steps = 16;
    for (let i = 1; i <= steps && !cancelled; i++) {
      await sleep(ms / steps);
      const done = Math.round((total * i) / steps);
      onProgress({ done, total, unit, rows: unit === 'rows' ? done : 0, bytes: unit === 'tables' ? done * 1.1 * MB : 0, stage: null });
    }
    jobs.delete(id);
    return cancelled;
  }

  return {
    ...rowsMock(profile, jobs),
    pickSave: async (name: string) => `/home/ada/Scrivania/${name}`,
    pickOpen: async (extensions: string[]) => (extensions.includes('dump') ? '/home/ada/Backup/gestionale-2026-10-27.dump' : '/home/ada/Scaricati/clienti-nuovi.csv'),
    pickFolder: async (purpose: 'clidriver' | 'tools' = 'tools') => (purpose === 'clidriver' ? '/opt/ibm/clidriver' : '/opt/postgresql/16/bin'),
    jobCancel: async (id: string) => jobs.get(id)?.(),
    toolDirSet: async () => {},
    dataExport: async (req: { job_id: string; expected_rows?: number | null; path: string }, onProgress: (p: Progress) => void) => {
      const total = Math.round(req.expected_rows ?? 2400);
      const cancelled = await tick(req.job_id, total, 'rows', onProgress);
      return { rows: cancelled ? 0 : total, bytes: cancelled ? 0 : total * 132, elapsed_ms: 1600, truncated: false, cancelled };
    },
    dataImportSample: async (sid: string) => {
      await sleep(120);
      const sample = structuredClone(SAMPLE);
      if (sqlite(sid)) for (const c of sample.columns) c.type_name = { integer: 'INTEGER', decimal: 'NUMERIC', date: 'DATE' }[c.inferred as string] ?? 'TEXT';
      return sample;
    },
    dataImport: async (req: { job_id: string; session_id: string; confirm?: string | null; on_error: 'stop' | 'skip' }, onProgress: (p: Progress) => void) => {
      await admit(req.session_id, req.confirm);
      const total = 2_380_000;
      const cancelled = await tick(req.job_id, total, 'bytes', (p) => onProgress({ ...p, rows: Math.round(((p.done ?? 0) / total) * 18_400) }));
      const faults = [
        { line: 214, message: 'value too long for type character(2)' },
        { line: 977, message: 'invalid input syntax for type numeric: "12.000,00"' },
      ];
      const stop = req.on_error === 'stop';
      return { inserted: cancelled || stop ? 0 : 18_398, skipped: stop ? 1 : 2, faults: stop ? faults.slice(0, 1) : faults, faults_cut: false, committed: !cancelled && !stop, cancelled, elapsed_ms: 1600 };
    },
    schemaCompare: async () => {
      await sleep(250);
      return structuredClone(COMPARISON);
    },
    backupTools: async (sid: string): Promise<ToolCheck> => {
      await sleep(80);
      const base = { supported: true, server_side: false, can_restore: true, schema_only: true };
      if (sqlite(sid)) return { ...base, dump: null, restore: [], formats: ['file'], builtin: true, schema_only: false };
      if (duckdb(sid)) return { ...base, dump: null, restore: [], formats: ['folder'], builtin: true, schema_only: false };
      if (driver(sid) === 'sqlserver' || driver(sid) === 'clickhouse') return { ...base, dump: null, restore: [], formats: [driver(sid) === 'clickhouse' ? 'zip' : 'bak'], builtin: true, server_side: true, can_restore: false, schema_only: false };
      if (driver(sid) === 'cockroach') return { ...base, dump: null, restore: [], formats: [], builtin: true, supported: false, can_restore: false, schema_only: false };
      if (new URLSearchParams(location.search).has('notools')) return { ...base, dump: null, restore: [], formats: ['archive', 'sql'], builtin: false };
      return {
        ...base,
        dump: { name: 'pg_dump', path: '/usr/bin/pg_dump', version: 'pg_dump (PostgreSQL) 16.10' },
        restore: [
          { name: 'pg_restore', path: '/usr/bin/pg_restore', version: 'pg_restore (PostgreSQL) 16.10' },
          { name: 'psql', path: '/usr/bin/psql', version: 'psql (PostgreSQL) 16.10' },
        ],
        formats: ['archive', 'sql'],
        builtin: false,
      };
    },
    backupRun: async (req: { job_id: string; session_id: string }, onProgress: (p: Progress) => void) => {
      const lite = sqlite(req.session_id);
      const cancelled = await tick(req.job_id, lite ? 2310 : 12, lite ? 'pages' : 'tables', onProgress, 2000);
      return { bytes: cancelled ? 0 : 13_002_342, elapsed_ms: 2000, cancelled, notes: null };
    },
    backupRestore: async (req: { job_id: string; session_id: string; confirm?: string | null }, onProgress: (p: Progress) => void) => {
      await admit(req.session_id, req.confirm);
      const cancelled = await tick(req.job_id, 13_002_342, 'bytes', onProgress, 2000);
      return { bytes: 13_002_342, elapsed_ms: 2000, cancelled, notes: null };
    },
    serverActivity: async (sid: string): Promise<Activity> => {
      await sleep(60);
      if (local(sid)) throw unsupported();
      return { sessions: structuredClone(SESSIONS), locks: [{ waiting: '18342', blocking: '18297', object: 'public.fatture', mode: 'ShareLock', waited_ms: 4200 }].filter((l) => SESSIONS.some((s) => s.id === l.blocking)), partial: false, can_cancel: driver(sid) !== 'sqlserver' };
    },
    serverSizes: async (sid: string): Promise<Sizes> => {
      await sleep(60);
      if (!local(sid)) return structuredClone(SIZES);
      return { ...structuredClone(SIZES), usage_known: false, indexes: SIZES.indexes.map((i) => ({ ...i, scans: null, unused: false })) };
    },
    serverKill: async (sid: string, target: string, _mode: string, confirm: string | null) => {
      await admit(sid, confirm);
      const i = SESSIONS.findIndex((s) => s.id === target);
      if (i >= 0) SESSIONS.splice(i, 1);
      for (const s of SESSIONS) s.blocked_by = s.blocked_by.filter((b) => b !== target);
      return true;
    },
    principalsList: async (sid: string): Promise<Principals> => {
      await sleep(60);
      if (local(sid)) throw unsupported();
      return structuredClone(PRINCIPALS);
    },
  };
}
