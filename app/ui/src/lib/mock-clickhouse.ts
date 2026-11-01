import type { Api, ConnectionProfile, RunEvent, RunRequest, Schema } from './api';
import type { Activity_Serialize as Activity, Capabilities, Monitor_Serialize as Monitor, ObjectRef_Deserialize as ObjectRef, Page_Serialize as Page, Sizes_Serialize as Sizes, SlowQueries_Serialize as SlowQueries } from './bindings';
import { relationalCaps } from './family';
import { DEFAULT_SESSION } from './session';

const params = () => new URLSearchParams(typeof location === 'undefined' ? '' : location.search);
const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));
const MB = 1024 * 1024;

export function clickhouseCaps(): Capabilities {
  return {
    ...relationalCaps('clickhouse'),
    tree: { databases: true, namespaces: 'multiple', groups: ['tables', 'views', 'materialized_views', 'dictionaries', 'functions'], server: true },
    transactions: false,
    editable: false,
    er: false,
    tools: ['activity', 'monitor', 'space', 'users', 'compare', 'rows', 'backup', 'transfer', 'seclab', 'search', 'generate'],
  };
}

export const CLICKHOUSE_DEMO: ConnectionProfile[] = params().has('clickhouse')
  ? [
      {
        id: 'clickhouse',
        name: 'Analisi vendite',
        folder: null,
        environment: params().get('clickhouse') === 'prod' ? 'production' : 'development',
        color: null,
        session: DEFAULT_SESSION,
        template: { kind: 'none' },
        raw_fields: {},
        target: { driver: 'clickhouse', host: '127.0.0.1', port: 58123, database: 'gestionale', user: 'nicedb', tls: 'disable', ca_file: null, ssh: null, proxy: null, client_cert: null },
      },
    ]
  : [];

const col = (name: string, type_name: string, primary_key = false) => ({ name, type_name, nullable: type_name.startsWith('Nullable('), primary_key });
const key = (name: string, columns: string[]) => ({ name, columns, unique: false });

export function clickhouseSchema(database: string): Schema {
  const databases = ['INFORMATION_SCHEMA', 'archivio', 'default', 'gestionale', 'information_schema', 'system'];
  if (database !== 'gestionale') {
    const tables = database === 'archivio' ? [{ name: 'log_eventi', columns: [col('istante', 'DateTime64(3)', true), col('livello', 'LowCardinality(String)'), col('messaggio', 'String')], indexes: [key('ORDER BY', ['istante']), key('PARTITION BY', ['toYYYYMM(istante)'])], foreign_keys: [], estimated_rows: 50000, detail: 'MergeTree' }] : [];
    return { databases, current_database: database, namespaces: [{ name: database, tables, views: [], functions: [], sequences: [], types: [], triggers: [] }] };
  }
  const righe = [col('id', 'UInt64'), col('ordine_id', 'UInt32', true), col('data', 'Date', true), col('articolo_id', 'UInt32'), col('quantita', 'Decimal(10, 2)'), col('prezzo', 'Decimal(10, 2)'), col('sconto', 'Decimal(4, 2)')];
  return {
    databases,
    current_database: database,
    namespaces: [
      {
        name: database,
        tables: [
          { name: 'agenti', columns: [col('id', 'UInt32', true), col('nome', 'String'), col('cognome', 'String'), col('zona', 'LowCardinality(String)'), col('provvigione', 'Decimal(4, 2)')], indexes: [key('ORDER BY', ['id'])], foreign_keys: [], estimated_rows: 8, detail: 'MergeTree' },
          { name: 'articoli', columns: [col('id', 'UInt32', true), col('codice', 'String'), col('descrizione', 'String'), col('um', 'LowCardinality(String)'), col('prezzo_listino', 'Decimal(10, 2)'), col('aliquota_iva', 'UInt8'), col('giacenza', 'Int32')], indexes: [key('ORDER BY', ['id'])], foreign_keys: [], estimated_rows: 1500, detail: 'MergeTree' },
          { name: 'clienti', columns: [col('id', 'UInt32', true), col('ragione_sociale', 'String'), col('partita_iva', 'FixedString(11)'), col('citta', 'LowCardinality(String)'), col('provincia', 'LowCardinality(String)'), col('agente_id', 'UInt32'), col('fido', 'Decimal(12, 2)'), col('creato_il', "DateTime('Europe/Rome')"), col('aggiornato_il', "DateTime('Europe/Rome')")], indexes: [key('ORDER BY', ['id'])], foreign_keys: [], estimated_rows: 2400, detail: 'ReplacingMergeTree' },
          { name: 'ordini', columns: [col('id', 'UInt32', true), col('numero', 'String'), col('data', 'Date', true), col('cliente_id', 'UInt32'), col('stato', "Enum8('bozza' = 1, 'confermato' = 2, 'spedito' = 3, 'chiuso' = 4, 'annullato' = 5)"), col('totale', 'Decimal(12, 2)')], indexes: [key('ORDER BY', ['data', 'id']), key('PARTITION BY', ['toYear(data)'])], foreign_keys: [], estimated_rows: 400000, detail: 'MergeTree' },
          { name: 'righe_ordine', columns: righe, indexes: [key('ORDER BY', ['data', 'ordine_id', 'articolo_id']), key('PRIMARY KEY', ['data', 'ordine_id']), key('PARTITION BY', ['toYear(data)'])], foreign_keys: [], estimated_rows: 3200000, detail: 'MergeTree' },
          { name: 'vendite_mensili', columns: [col('mese', 'Date', true), col('articolo_id', 'UInt32', true), col('quantita', 'Decimal(18, 2)'), col('importo', 'Decimal(18, 2)')], indexes: [key('ORDER BY', ['mese', 'articolo_id'])], foreign_keys: [], estimated_rows: 90000, detail: 'SummingMergeTree' },
        ],
        views: [{ name: 'v_ordini_aperti', columns: [col('numero', 'String'), col('data', 'Date'), col('ragione_sociale', 'String'), col('stato', 'String')], indexes: [], foreign_keys: [], estimated_rows: null }],
        materialized_views: [{ name: 'mv_vendite_mensili', columns: [col('mese', 'Date'), col('articolo_id', 'UInt32'), col('quantita', 'Decimal(38, 2)'), col('importo', 'Decimal(38, 6)')], indexes: [], foreign_keys: [], estimated_rows: null, detail: '→ vendite_mensili' }],
        dictionaries: [{ name: 'dict_articoli', columns: [col('id', 'UInt64', true), col('codice', 'String'), col('descrizione', 'String'), col('prezzo_listino', 'Decimal(10, 2)')], indexes: [], foreign_keys: [], estimated_rows: 1500, detail: 'Hashed · LOADED' }],
        functions: [],
        sequences: [],
        types: [],
        triggers: [],
      },
    ],
  };
}

const DDL: Record<string, string> = {
  righe_ordine: `CREATE TABLE gestionale.righe_ordine
(
    \`id\` UInt64,
    \`ordine_id\` UInt32,
    \`data\` Date,
    \`articolo_id\` UInt32,
    \`quantita\` Decimal(10, 2),
    \`prezzo\` Decimal(10, 2),
    \`sconto\` Decimal(4, 2)
)
ENGINE = MergeTree
PARTITION BY toYear(data)
PRIMARY KEY (data, ordine_id)
ORDER BY (data, ordine_id, articolo_id)
SETTINGS index_granularity = 8192
COMMENT 'Righe degli ordini, una per articolo';
`,
};

const STATI = ['bozza', 'confermato', 'spedito', 'chiuso', 'chiuso', 'chiuso', 'annullato'];
const day = (o: number) => new Date(Date.UTC(2021, 0, 1) + Math.floor(o / 220) * 86400000).toISOString().slice(0, 10);

function rowsOf(table: string, from: number, count: number): { columns: [string, string][]; rows: (string | null)[][] } {
  if (table === 'ordini') {
    return {
      columns: [['id', 'UInt32'], ['numero', 'String'], ['data', 'Date'], ['cliente_id', 'UInt32'], ['stato', 'Enum8'], ['totale', 'Decimal(12, 2)']],
      rows: Array.from({ length: count }, (_, k) => {
        const i = from + k + 1;
        return [String(i), `${day(i).slice(0, 4)}/${String(i).padStart(6, '0')}`, day(i), String(1 + ((i * 31) % 2400)), STATI[i % 7] ?? 'bozza', (((i * 37) % 9000) + 120.5).toFixed(2)];
      }),
    };
  }
  return {
    columns: [['id', 'UInt64'], ['ordine_id', 'UInt32'], ['data', 'Date'], ['articolo_id', 'UInt32'], ['quantita', 'Decimal(10, 2)'], ['prezzo', 'Decimal(10, 2)'], ['sconto', 'Decimal(4, 2)']],
    rows: Array.from({ length: count }, (_, k) => {
      const n = from + k;
      const o = 1 + Math.floor(n / 8);
      return [String(n + 1), String(o), day(o), String(1 + ((n * 7) % 1500)), (1 + (n % 24)).toFixed(2), (0.15 + ((n * 7) % 400) * 0.37).toFixed(2), n % 9 === 0 ? '10.00' : '0.00'];
    }),
  };
}

const running = new Map<string, () => void>();

function statements(text: string): { from: number; to: number; sql: string }[] {
  const out: { from: number; to: number; sql: string }[] = [];
  let start = 0;
  for (let i = 0; i <= text.length; i++) {
    if (i === text.length || text[i] === ';') {
      const raw = text.slice(start, i);
      const lead = raw.length - raw.trimStart().length;
      const sql = raw.trim();
      if (sql) out.push({ from: start + lead, to: start + lead + sql.length, sql });
      start = i + 1;
    }
  }
  return out;
}

async function run(req: RunRequest, emit: (e: RunEvent) => void) {
  let cancelled = false;
  running.set(req.run_id, () => (cancelled = true));
  const started = performance.now();
  const all = statements(req.text);
  const s = req.scope;
  const chosen = s.kind === 'script' ? all : s.kind === 'statement' ? [all.find((x) => x.from <= s.cursor && s.cursor <= x.to) ?? all[all.length - 1]].filter((x) => !!x) : all.filter((x) => x.to > Math.min(s.from, s.to) && x.from < Math.max(s.from, s.to));
  for (const [index, st] of chosen.entries()) {
    if (cancelled) break;
    emit({ type: 'statement', index, from: st.from, to: st.to, sql: st.sql, unsortable: null });
    const slow = /\bsum\(|\bgroup by\b|count\(\)/i.test(st.sql);
    if (slow) {
      const total = 3_200_000;
      const hold = params().has('hold');
      for (let step = 1; step <= (hold ? 1000 : 6) && !cancelled; step++) {
        const read = Math.min(total, hold ? 1_380_000 : Math.round((total * step) / 6));
        emit({ type: 'progress', index, progress: { read_rows: read, read_bytes: read * 22, total_rows: total } });
        await sleep(hold ? 1000 : 140);
      }
      if (cancelled) break;
      emit({ type: 'columns', index, columns: [{ name: 'mese', type_name: 'Date' }, { name: 'righe', type_name: 'UInt64' }, { name: 'importo', type_name: 'Decimal(38, 2)' }] });
      emit({ type: 'rows', index, rows: Array.from({ length: 12 }, (_, i) => [`2025-${String(i + 1).padStart(2, '0')}-01`, String(52000 + ((i * 7919) % 9000)), (812345.5 + i * 18234.25).toFixed(2)]) });
      emit({ type: 'done', index, elapsed_ms: 840, row_count: 12, rows_affected: null, command: null, truncated: false, row_limit: null });
      continue;
    }
    const table = /\bfrom\s+`?(\w+)`?/i.exec(st.sql)?.[1] ?? 'righe_ordine';
    const limit = Number(/\blimit\s+(\d+)/i.exec(st.sql)?.[1] ?? 1000);
    const got = rowsOf(table, 0, Math.min(limit, 2000));
    if (!/^\s*select\b/i.test(st.sql)) {
      emit({ type: 'done', index, elapsed_ms: 12, row_count: 0, rows_affected: /^\s*insert\b/i.test(st.sql) ? 3 : null, command: (/^\s*(\w+)/.exec(st.sql)?.[1] ?? '').toUpperCase(), truncated: false, row_limit: null });
      continue;
    }
    emit({ type: 'columns', index, columns: got.columns.map(([name, type_name]) => ({ name, type_name })) });
    emit({ type: 'rows', index, rows: got.rows });
    emit({ type: 'done', index, elapsed_ms: 38, row_count: got.rows.length, rows_affected: null, command: null, truncated: false, row_limit: null });
  }
  running.delete(req.run_id);
  emit({ type: 'finished', elapsed_ms: Math.round(performance.now() - started), cancelled });
}

const SIZES: Sizes = {
  usage_known: false,
  tables: [
    { namespace: 'gestionale', name: 'righe_ordine', rows: 3_200_000, data_bytes: 15.2 * MB, index_bytes: 0.06 * MB, total_bytes: 16.98 * MB },
    { namespace: 'gestionale', name: 'ordini', rows: 400_000, data_bytes: 3.1 * MB, index_bytes: 0.01 * MB, total_bytes: 3.62 * MB },
    { namespace: 'archivio', name: 'log_eventi', rows: 50_000, data_bytes: 0.41 * MB, index_bytes: 0.002 * MB, total_bytes: 0.47 * MB },
    { namespace: 'gestionale', name: 'vendite_mensili', rows: 90_000, data_bytes: 0.4 * MB, index_bytes: 0.002 * MB, total_bytes: 0.47 * MB },
    { namespace: 'gestionale', name: 'clienti', rows: 2400, data_bytes: 0.045 * MB, index_bytes: 0.0001 * MB, total_bytes: 0.05 * MB },
  ],
  indexes: [],
  partitions: [
    ...['2021', '2022', '2023', '2024', '2025', '2026'].map((y, i) => ({ namespace: 'gestionale', table: 'ordini', partition: y, parts: i === 5 ? 3 : 1, rows: i === 5 ? 18_200 : 80_300, bytes: (i === 5 ? 0.17 : 0.69) * MB })),
    ...['2021', '2022', '2023', '2024', '2025', '2026'].map((y, i) => ({ namespace: 'gestionale', table: 'righe_ordine', partition: y, parts: i === 5 ? 2 : 1, rows: i === 5 ? 145_600 : 642_400, bytes: (i === 5 ? 0.78 : 3.24) * MB })),
  ],
};

const MONITOR: Monitor = {
  connections: { total: 6, active: 2, idle_in_transaction: null, waiting: null, max: 4096 },
  transactions: [],
  databases: [
    { name: 'gestionale', bytes: 21.7 * MB },
    { name: 'system', bytes: 48.2 * MB },
    { name: 'archivio', bytes: 0.47 * MB },
  ],
  cache_hit: 0.982,
  merges: [
    { namespace: 'gestionale', table: 'righe_ordine', elapsed_ms: 4200, progress: 0.64, parts: 6, bytes: 14.1 * MB, mutation: false },
    { namespace: 'gestionale', table: 'clienti', elapsed_ms: 900, progress: 0.18, parts: 1, bytes: 0.05 * MB, mutation: true },
  ],
  gaps: [{ part: 'transactions', reason: { kind: 'not_exposed' } }],
};

const SLOW: SlowQueries = {
  source: 'query_log',
  gap: null,
  queries: [
    { text: 'SELECT toStartOfMonth(data) AS mese, count(), sum(quantita * prezzo) FROM righe_ordine GROUP BY mese ORDER BY mese', sample: 'SELECT toStartOfMonth(data) AS mese, count(), sum(quantita * prezzo) FROM righe_ordine GROUP BY mese ORDER BY mese', calls: 14, total_ms: 9120, mean_ms: 651, rows: 1008 },
    { text: 'SELECT * FROM ordini WHERE cliente_id = ? ORDER BY data DESC LIMIT ?', sample: 'SELECT * FROM ordini WHERE cliente_id = 42 ORDER BY data DESC LIMIT 50', calls: 230, total_ms: 3910, mean_ms: 17, rows: 11500 },
  ],
};

const ACTIVITY: Activity = {
  sessions: [
    { id: '7f3c1a52-0d4e-4b1a-9a61-2b8e0c3f11d0', user: 'nicedb', database: 'gestionale', client: '::ffff:10.0.4.21', application: 'NiceDB', state: 'active', query: 'SELECT articolo_id, sum(quantita) FROM righe_ordine GROUP BY articolo_id', duration_ms: 2140, wait: null, blocked_by: [] },
    { id: 'etl-notturno-20261029', user: 'etl', database: 'gestionale', client: '::ffff:10.0.4.30', application: 'clickhouse-client', state: 'active', query: 'INSERT INTO righe_ordine SELECT * FROM s3(…)', duration_ms: 48200, wait: null, blocked_by: [] },
  ],
  locks: [],
  partial: false,
  can_cancel: true,
};

export function clickhouseMock(base: Api, profile: (sessionId: string) => ConnectionProfile | undefined, database: (sessionId: string) => string): Partial<Api> {
  const mine = (sid: string) => profile(sid.split('@')[0] ?? sid)?.target.driver === 'clickhouse';
  return {
    schemaLoad: async (sid) => (mine(sid) ? (await sleep(60), clickhouseSchema(database(sid))) : base.schemaLoad(sid)),
    objectDdl: async (sid, object: ObjectRef) => (mine(sid) ? (DDL[object.name] ?? `CREATE TABLE ${object.namespace}.${object.name}\n(\n    \`id\` UInt32\n)\nENGINE = MergeTree\nORDER BY id;\n`) : base.objectDdl(sid, object)),
    run: (req, emit) => (mine(req.session_id) ? run(req, emit) : base.run(req, emit)),
    runCancel: async (id) => {
      const stop = running.get(id);
      if (stop) return stop();
      return base.runCancel(id);
    },
    dataBrowse: async (sid, b): Promise<Page> => {
      if (!mine(sid)) return base.dataBrowse(sid, b);
      await sleep(50);
      const got = rowsOf(b.table.name, b.offset ?? 0, b.limit ?? 100);
      return { columns: got.columns.map(([name, type_name]) => ({ name, type_name })), rows: got.rows, has_more: true, next: null, total: null, estimated: false, sql: `SELECT * FROM ${b.table.namespace}.${b.table.name} LIMIT ${b.limit}`, elapsed_ms: 42 };
    },
    serverSizes: async (sid) => (mine(sid) ? structuredClone(SIZES) : base.serverSizes(sid)),
    serverMonitor: async (sid) => (mine(sid) ? structuredClone(MONITOR) : base.serverMonitor(sid)),
    serverSlowQueries: async (sid) => (mine(sid) ? structuredClone(SLOW) : base.serverSlowQueries(sid)),
    serverActivity: async (sid) => (mine(sid) ? structuredClone(ACTIVITY) : base.serverActivity(sid)),
  };
}
