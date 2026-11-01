import type { Api, Appearance, ConnectionProfile, HistoryEntry, RunEvent, RunRequest, Schema, Secrets, Visibility } from './api';
import { erApi, erSchema } from './mock-er';
import { withObjects } from './mock-explorer';
import { dataMock } from './mock-data';
import { stressTables } from './mock-fix';
import { planMock } from './mock-plan';
import { ENGINE_CONNECTIONS, enginePlan } from './mock-extras';
import { previewScale, sortingRun } from './mock-ux';
import { orgMock } from './mock-org';
import { dialectOf } from './drivers';
import { REDIS_DEMO, redisRead, redisReport, redisRun, redisScan, redisSchema, redisWrite } from './mock-redis';
import { capsOf, MONGO_DEMO, mongoCancel, mongoDdl, mongoRun, mongoSchema, mongoWrite } from './mock-families';
import { mockServer } from './mock-drivers';
import { CLICKHOUSE_DEMO, clickhouseMock } from './mock-clickhouse';
import { NOSQL2_DEMO, nosql2Mock } from './mock-nosql2';
import { CLUSTER_DEMO, extras3Mock } from './mock-extras3';
import { mockDdl } from './mock-ddl';
import { libraryMock } from './mock-notes';
import { seclabMock } from './mock-seclab';
import { toolsMock } from './mock-tools';
import { sessionMock } from './mock-session';
import { fkMock, fkTables, withForeignKeys } from './mock-b5';
import { txMock } from './mock-b3';
import { marksMock } from './mock-b11';
import { monitorMock } from './mock-b12';
import { valueTables } from './mock-b7';
import { chartTables } from './mock-b8';
import { paramsMock } from './mock-b4';
import { searchMock } from './mock-b14';
import { generateMock } from './mock-b13';
import { genProfilesMock } from './mock-polish';
import { b6Schema } from './mock-b6';
import { b2Mock } from './mock-b2';
import { b10gMock } from './mock-b10g';
import { b15Mock } from './mock-b15';
import { DEFAULT_SESSION } from './session';
import { bigSchema, overIpc } from './mock-performance';
import { mssqlMock } from './mock-mssql';
import { chaosCalls, chaosNet, monkeyTables } from './mock-monkey';
import { clickhouseGenMock, lazyMock } from './mock-extras7';
import { SHOWCASE_CONNECTIONS, SHOWCASE_DB, SHOWCASE_TABLES, showcasePlan, showcaseSchema, showcaseValue } from './mock-showcase';
import { aboutMockFromUrl } from './mock-about';
import { OTHER_DEMO, otherdbMock } from './mock-otherdb';
import { extras6Mock } from './mock-extras6';
import { ORACLE_DEMO, oracleMock } from './mock-oracle';
import { ENTERPRISE_DEMO, enterpriseMock } from './mock-enterprise';
import { EXTRAS8_DEMO, extras8Mock } from './mock-extras8';
import { EXTRAS4_DEMO, extras4Mock, extras4Tables } from './mock-extras4';
import { extras5Mock } from './mock-extras5';
import { acceptanceMock } from './mock-acceptance';
import { englishMock } from './mock-en';

const KEY = 'nicedb.appearance';
const params = () => new URLSearchParams(typeof location === 'undefined' ? '' : location.search);
const SHOWCASE = params().has('showcase');
const MAIN_DB = SHOWCASE ? SHOWCASE_DB : 'gestionale';

const DEMO: ConnectionProfile[] = [
  {
    id: 'bassi-prod',
    name: 'Bassi — produzione',
    folder: 'f-bassi',
    environment: 'production',
    color: null,
    session: DEFAULT_SESSION, template: { kind: 'none' }, raw_fields: {},
    target: { driver: 'postgres', host: 'db.bassi.lan', port: 5432, database: 'gestionale', user: 'lettura', tls: 'verify', ca_file: null, ssh: { host: 'bastion.bassi.it', port: 22, user: 'ada', auth: { method: 'agent' }, jumps: [] }, proxy: null, client_cert: null },
  },
  {
    id: 'bassi-staging',
    name: 'Bassi — staging',
    folder: 'f-bassi',
    environment: 'staging',
    color: null,
    session: DEFAULT_SESSION, template: { kind: 'none' }, raw_fields: {},
    target: { driver: 'postgres', host: 'staging.bassi.lan', port: 5432, database: 'gestionale', user: 'ada', tls: 'verify', ca_file: null, ssh: null, proxy: null, client_cert: null },
  },
  {
    id: 'bassi-locale',
    name: 'Bassi — locale',
    folder: 'f-bassi',
    environment: 'local',
    color: null,
    session: DEFAULT_SESSION, template: { kind: 'none' }, raw_fields: {},
    target: { driver: 'sqlite', path: '/home/ada/dati/bassi.sqlite', read_only: false },
  },
  {
    id: 'marini',
    name: 'Studio Marini — gestionale',
    folder: 'f-marini',
    environment: 'development',
    color: null,
    session: DEFAULT_SESSION, template: { kind: 'none' }, raw_fields: {},
    target: { driver: 'postgres', host: '127.0.0.1', port: 55432, database: 'gestionale', user: 'nicedb', tls: 'disable', ca_file: null, ssh: null, proxy: null, client_cert: null },
    ...(params().has('limit') ? { session: { ...DEFAULT_SESSION, init: "SET search_path TO vendite, public;\nSET TIME ZONE 'Europe/Rome';", keepalive_secs: 120, row_limit: 1000 } } : {}),
  },
];

const trashed = new Map<string, ConnectionProfile>();
if (typeof window !== 'undefined') (window as unknown as { __ndTrashed?: () => string[] }).__ndTrashed = () => [...trashed.keys()];
let connections: ConnectionProfile[] = [...(params().has('demo') ? structuredClone([...(SHOWCASE ? SHOWCASE_CONNECTIONS : DEMO), ...(params().has('engines') ? ENGINE_CONNECTIONS : [])]) : []), ...structuredClone(MONGO_DEMO), ...structuredClone(REDIS_DEMO), ...structuredClone(CLICKHOUSE_DEMO), ...structuredClone(ORACLE_DEMO), ...structuredClone(ENTERPRISE_DEMO), ...structuredClone(EXTRAS8_DEMO), ...structuredClone(CLUSTER_DEMO), ...structuredClone(NOSQL2_DEMO), ...(params().has('other') ? structuredClone(OTHER_DEMO) : []), ...structuredClone(EXTRAS4_DEMO)];
const history: HistoryEntry[] = [];
const running = new Map<string, () => void>();

const CITIES = ['Bergamo', 'Brescia', 'Seriate', 'Lumezzane', 'Dalmine', 'Treviglio', 'Chiari', 'Rovato'];
const KINDS = ['Edilnord', 'Termoidraulica', 'Officine Meccaniche', 'Serramenti', 'Carpenteria', 'Falegnameria'];
const FAMILIES = ['Bassi', 'Valtrompia', 'Gatti', 'Pedrali', 'Locatelli', 'Bonomi'];
const FORMS = ['S.r.l.', 'S.p.A.', 'S.n.c.', 'S.a.s.'];

type Gen = { columns: [string, string][]; total: number; row: (i: number) => (string | null)[] };

const TABLES: Record<string, Gen> = SHOWCASE ? SHOWCASE_TABLES : {
  clienti: {
    columns: [['id', 'int4'], ['ragione_sociale', 'text'], ['partita_iva', 'bpchar'], ['citta', 'text'], ['provincia', 'bpchar'], ['fido', 'numeric']],
    total: 2400,
    row: (i) => [
      String(i),
      `${KINDS[i % 6]} ${FAMILIES[(i >> 3) % 6]} ${FORMS[i % 4]}`,
      String(10000000000 + i * 7919),
      CITIES[i % 8] ?? '',
      i % 2 ? 'BG' : 'BS',
      `${(5 + (i % 20)) * 1000}.00`,
    ],
  },
  righe_ordine: {
    columns: [['id', 'int8'], ['ordine_id', 'int4'], ['articolo_id', 'int4'], ['quantita', 'numeric'], ['prezzo', 'numeric'], ['sconto', 'numeric']],
    total: 1_200_000,
    row: (i) => [String(i), String(1 + (i % 120000)), String(1 + ((i * 7) % 4000)), `${1 + (i % 24)}.00`, (0.15 + (i % 400) * 0.37).toFixed(2), i % 6 > 2 ? '10.00' : '0.00'],
  },
  fatture: {
    columns: [['numero', 'text'], ['data', 'date'], ['cliente_id', 'int4'], ['totale', 'numeric'], ['scadenza', 'date'], ['pagata', 'bool'], ['note', 'jsonb'], ...(params().has('bin') ? [['firma', 'bytea'] as [string, string]] : [])],
    total: 90000,
    row: (i) => [`FT${String(i).padStart(6, '0')}`, '2026-03-14', String(1 + ((i * 13) % 2400)), (97.6 + ((i * 97) % 9000) * 1.22).toFixed(2), '2026-05-13', i % 5 ? 't' : 'f', i % 11 ? null : '{"canale": "pec", "sollecito": 1}', i % 3 ? null : `\\x89504e47${(i * 2654435761).toString(16).padStart(8, '0').slice(-8)}`],
  },
  ...stressTables(params()),
  ...fkTables(params()),
  ...valueTables(params()),
  ...chartTables(params()),
  ...monkeyTables(params()),
  ...extras4Tables(params()),
};

const sessions = sessionMock((sid) => connections.find((c) => `mock-${c.id}` === sid.split('@')[0]));

const { guardRun, ...data } = dataMock(TABLES, (sid) => connections.find((c) => `mock-${c.id}` === sid.split('@')[0]));
const prm = paramsMock(params().has('demo'));
const tx = txMock({ tables: TABLES, profile: (sid) => connections.find((c) => `mock-${c.id}` === sid.split('@')[0]), access: data.writeAccess });

const SCHEMA: Schema = SHOWCASE ? showcaseSchema() : {
  databases: ['gestionale', 'gestionale_archivio', 'postgres', 'template0', 'template1'],
  current_database: 'gestionale',
  namespaces: [
    {
      name: 'public',
      tables: Object.entries(TABLES).map(([name, g]) => ({
        name,
        estimated_rows: g.total,
        columns: g.columns.map(([n, t], i) => ({ name: n, type_name: t, nullable: i > 1, primary_key: i === 0 })),
        indexes: [{ name: `${name}_pkey`, columns: [g.columns[0]?.[0] ?? 'id'], unique: true }],
        foreign_keys: [],
      })),
      views: [{ name: 'v_scadenzario', estimated_rows: null, columns: [{ name: 'numero', type_name: 'text', nullable: true, primary_key: false }], indexes: [], foreign_keys: [] }],
      functions: [{ name: 'totale_cliente', signature: 'cid integer' }],
      sequences: [],
      types: [],
      triggers: [],
    },
  ],
};

const ARCHIVE: Schema['namespaces'] = [
  {
    name: 'public',
    tables: [
      { name: 'ordini_storici', estimated_rows: 840000, columns: [{ name: 'id', type_name: 'int8', nullable: false, primary_key: true }, { name: 'cliente_id', type_name: 'int4', nullable: false, primary_key: false }, { name: 'archiviato_il', type_name: 'date', nullable: true, primary_key: false }], indexes: [{ name: 'ordini_storici_pkey', columns: ['id'], unique: true }], foreign_keys: [] },
      { name: 'fatture_storiche', estimated_rows: 310000, columns: [{ name: 'numero', type_name: 'text', nullable: false, primary_key: true }, { name: 'totale', type_name: 'numeric', nullable: true, primary_key: false }], indexes: [{ name: 'fatture_storiche_pkey', columns: ['numero'], unique: true }], foreign_keys: [] },
    ],
    views: [],
    functions: [], sequences: [], types: [], triggers: [],
  },
  { name: 'audit', tables: [{ name: 'eventi', estimated_rows: 52000, columns: [{ name: 'id', type_name: 'int8', nullable: false, primary_key: true }, { name: 'evento', type_name: 'text', nullable: true, primary_key: false }], indexes: [{ name: 'eventi_pkey', columns: ['id'], unique: true }], foreign_keys: [] }], views: [], functions: [], sequences: [], types: [], triggers: [] },
  { name: 'information_schema', tables: [{ name: 'tables', estimated_rows: null, columns: [{ name: 'table_name', type_name: 'text', nullable: true, primary_key: false }], indexes: [], foreign_keys: [] }], views: [], functions: [], sequences: [], types: [], triggers: [] },
];

const sessionDb = new Map<string, string>();
const isRedis = (sid: string) => connections.find((c) => `mock-${c.id}` === sid.split('@')[0])?.target.driver === 'redis';
const isMongo = (sid: string) => connections.find((c) => `mock-${c.id}` === sid.split('@')[0])?.target.driver === 'mongodb';
const visibility = new Map<string, Visibility>();

function utf16Ranges(text: string): { from: number; to: number; sql: string }[] {
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

function pick(req: RunRequest) {
  const all = utf16Ranges(req.text);
  const s = req.scope;
  if (s.kind === 'script') return all;
  if (s.kind === 'selection') return all.filter((r) => r.to > Math.min(s.from, s.to) && r.from < Math.max(s.from, s.to));
  const at = [...all].reverse().find((r) => r.from <= s.cursor) ?? all[0];
  return at ? [at] : [];
}

const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));

async function run(req: RunRequest, emit: (e: RunEvent) => void) {
  let cancelled = false;
  running.set(req.run_id, () => (cancelled = true));
  const started = performance.now();
  const statements = pick(req);
  for (const [index, st] of statements.entries()) {
    if (cancelled) break;
    emit({ type: 'statement', index, from: st.from, to: st.to, sql: st.sql, unsortable: null });
    const t = performance.now();
    const bad = st.sql.search(/città|\bfro\b/);
    if (bad >= 0) {
      const at = st.from + bad;
      const before = req.text.slice(0, at);
      const line = before.split('\n').length;
      const column = at - before.lastIndexOf('\n');
      emit({
        type: 'failed',
        index,
        elapsed_ms: 2,
        error: { kind: 'query', message: 'la colonna «città» non esiste', code: '42703', detail: null, hint: 'Forse intendevi «clienti.citta».', offset: bad },
        position: { line, column },
        at,
      });
      if (req.continue_on_error) continue;
      break;
    }
    const table = Object.keys(TABLES).find((name) => new RegExp(`\\bfrom\\s+${name}\\b`, 'i').test(st.sql));
    const gen = table ? TABLES[table] : undefined;
    if (!gen) {
      emit({ type: 'columns', index, columns: [{ name: '?column?', type_name: 'int4' }] });
      emit({ type: 'rows', index, rows: [['1']] });
      emit({ type: 'done', index, elapsed_ms: 1, row_count: 1, rows_affected: null, command: 'SELECT', truncated: false, row_limit: null });
      continue;
    }
    const limit = Number(/\blimit\s+(\d+)/i.exec(st.sql)?.[1] ?? gen.total);
    const total = Math.min(limit, gen.total);
    emit({ type: 'columns', index, columns: gen.columns.map(([name, type_name]) => ({ name, type_name })) });
    let sent = 0;
    while (sent < total && !cancelled) {
      const n = Math.min(sent === 0 ? 200 : 4000, total - sent);
      emit({ type: 'rows', index, rows: Array.from({ length: n }, (_, k) => gen.row(sent + k + 1)) });
      sent += n;
      await sleep(sent < 1000 ? 30 : 4);
    }
    if (cancelled) break;
    emit({ type: 'done', index, elapsed_ms: Math.round(performance.now() - t), row_count: sent, rows_affected: null, command: 'SELECT', truncated: false, row_limit: null });
    history.unshift({ id: history.length + 1, connection_id: req.session_id, sql: st.sql, started_at: Date.now(), elapsed_ms: Math.round(performance.now() - t), rows: sent, error: null });
  }
  running.delete(req.run_id);
  emit({ type: 'finished', elapsed_ms: Math.round(performance.now() - started), cancelled });
}

function readAppearance(): Appearance {
  let stored: Partial<Appearance> = {};
  try {
    stored = JSON.parse(localStorage.getItem(KEY) ?? '{}') as Partial<Appearance>;
  } catch {
    stored = {};
  }
  const url = Object.fromEntries(['style', 'palette', 'theme', 'locale'].flatMap((k) => (params().get(k) ? [[k, params().get(k)]] : [])));
  const scale = Number(params().get('scale')) || undefined;
  const a: Appearance = {
    style: 'listino',
    palette: '2',
    theme: 'dark',
    locale: 'it',
    ui_scale: 100,
    sounds: true,
    sound_volume: 30,
    sound_detail: 'all',
    sound_timbre: 'marimba',
    sound_gestures: false,
    jingle: true,
    setup_done: true,
    ...(SHOWCASE ? {} : stored),
    ...(url as Partial<Appearance>),
    ...(scale ? { ui_scale: scale } : {}),
    ...(params().get('wizard') === '1' ? { setup_done: false } : {}),
  };
  previewScale(a.ui_scale);
  return a;
}

const tools = toolsMock({ profile: (sid) => connections.find((c) => `mock-${c.id}` === sid), access: data.writeAccess });

const sqlRun = prm.wrap(tx.wrap(sessions.limited((req, emit) => guardRun(req, emit, sortingRun(run)))));

const fk = fkMock(data.dataBrowse);
export const mock: Api = {
  appearanceGet: async () => readAppearance(),
  appearanceSet: async (v) => {
    previewScale(v.ui_scale);
    try {
      localStorage.setItem(KEY, JSON.stringify(v));
    } catch {
      return;
    }
  },
  connectionsList: async () => structuredClone(connections),
  connectionSave: async (p: ConnectionProfile, _s: Secrets) => {
    connections = [...connections.filter((c) => c.id !== p.id), structuredClone(p)];
  },
  connectionDelete: async (id) => {
    connections = connections.filter((c) => c.id !== id);
  },
  connectionTrash: async (id) => {
    const gone = connections.find((c) => c.id === id);
    if (gone) trashed.set(id, gone);
    connections = connections.filter((c) => c.id !== id);
  },
  connectionRestore: async (p) => {
    trashed.delete(p.id);
    connections = [...connections.filter((c) => c.id !== p.id), structuredClone(p)];
  },
  connectionPurge: async (id) => {
    trashed.delete(id);
  },
  connectionTest: async (p) => {
    await sleep(300);
    sessions.checkInit(p);
    return mockServer(p.target);
  },
  hostKeyTrust: async () => {},
  varsPreview: async () => [],
  sessionOpen: async (id, db = null) => {
    await sleep(120);
    const p = connections.find((c) => c.id === id);
    const target = p?.target ?? { driver: 'postgres', host: '', port: 0, database: '', user: '', tls: 'disable', ca_file: null, ssh: null, proxy: null, client_cert: null };
    const database = db ?? (target.driver === 'redis' ? mockServer(target).database : MAIN_DB);
    const session_id = db ? `mock-${id}@${db}` : `mock-${id}`;
    sessionDb.set(session_id, database);
    return { session_id, connection_id: id, environment: p?.environment ?? 'local', dialect: dialectOf(target.driver), caps: capsOf(target.driver), server: { ...mockServer(target), database }, transaction_lost: false, edition: null };
  },
  sessionReconnect: async (sid) => {
    await sleep(80);
    const [base, db] = sid.split('@');
    return mock.sessionOpen(base?.slice(5) ?? '', db ?? null);
  },
  sessionWatch: sessions.sessionWatch,
  sessionSwitch: async (sid, database) => {
    const id = sid.split('@')[0]?.slice(5) ?? '';
    return mock.sessionOpen(id, database);
  },
  visibilityGet: async (id) => structuredClone(visibility.get(id) ?? { databases: {}, schemas: {} }),
  visibilitySet: async (id, v) => {
    visibility.set(id, structuredClone(v));
  },
  sessionClose: async () => {},
  schemaLoad: async (sid) => {
    const database = sessionDb.get(sid) ?? MAIN_DB;
    if (isMongo(sid)) return (await sleep(60), mongoSchema(database));
    if (isRedis(sid)) return (await sleep(60), redisSchema(sessionDb.get(sid)));
    const base = structuredClone(SCHEMA);
    if (database === 'gestionale_archivio') return { ...base, current_database: database, namespaces: structuredClone(ARCHIVE) };
    if (database !== MAIN_DB) return { ...base, current_database: database, namespaces: [{ name: 'public', tables: [], views: [], functions: [], sequences: [], types: [], triggers: [] }] };
    const build = () => bigSchema(b6Schema(withForeignKeys(withObjects(erSchema(base, params().get('er')), params().get('objects')), params()), params()), params());
    return params().has('big') ? overIpc(location.search, build) : build();
  },
  schemaOpen: async (sid) => ({ schema: await mock.schemaLoad(sid), pending: [] }),
  schemaNamespace: async (sid, namespace) => {
    const all = await mock.schemaLoad(sid);
    return { ...all, namespaces: all.namespaces.filter((n) => n.name === namespace) };
  },
  searchPath: async () => null,
  fileLoad: async () => null,
  clickhouseAccess: async () => ({ profiles: [], policies: [] }),
  objectDdl: async (sid, object) => {
    await sleep(90);
    const p = connections.find((c) => `mock-${c.id}` === sid.split('@')[0]);
    if (isMongo(sid)) return mongoDdl(object);
    return mockDdl(dialectOf(p?.target.driver ?? 'postgres') ?? 'postgres', await mock.schemaLoad(sid), object);
  },
  ...b10gMock((sid) => mock.schemaLoad(sid)),
  ...b15Mock((sid) => mock.schemaLoad(sid)),
  run: (req, emit) => (isRedis(req.session_id) ? redisRun(sessionDb.get(req.session_id), req, emit) : isMongo(req.session_id) ? mongoRun(req, emit) : sqlRun(req, emit)),
  keysScan: (sid, scan) => redisScan(sessionDb.get(sid), scan),
  keyRead: (sid, read) => redisRead(sessionDb.get(sid), read),
  keyWrite: async (sid, write) => {
    const access = await data.writeAccess(sid);
    if (access.guarded && !access.open) throw { code: 'read_only' };
    return redisWrite(sessionDb.get(sid), write);
  },
  serverReport: () => redisReport(),
  documentsWrite: async (sid, write) => {
    const access = await data.writeAccess(sid);
    if (access.guarded && !access.open) throw { code: 'read_only' };
    return mongoWrite(write);
  },
  paramsFound: prm.paramsFound,
  ...data,
  ...fk,
  ...mssqlMock(fk.dataBrowse, (sid) => connections.find((c) => `mock-${c.id}` === sid.split('@')[0])?.target.driver === 'sqlserver'),
  dataPick: (sid, b) => mock.dataBrowse(sid, b),
  dataApply: tx.holdApply(data.dataApply),
  ...tx.api,
  runCancel: async (id) => {
    if (mongoCancel(id)) return;
    running.get(id)?.();
  },
  historyList: async (_c, q, limit) => history.filter((h) => h.sql.toLowerCase().includes(q.toLowerCase())).slice(0, limit),
  ...erApi,
  ...planMock(running, (sid) => enginePlan(connections.find((c) => `mock-${c.id}` === sid.split('@')[0])?.target.driver), SHOWCASE ? showcasePlan : undefined),
  ...orgMock(params().has('demo'), params().has('org'), () => connections, (next) => (connections = next)),
  ...tools,
  ...libraryMock(params().has('demo')),
  ...seclabMock({ profile: (sid) => connections.find((c) => `mock-${c.id}` === sid) }),
  ...marksMock(params().has('marks')),
  ...genProfilesMock(params().has('demo')),
  ...monitorMock({ profile: (sid) => connections.find((c) => `mock-${c.id}` === sid), access: data.writeAccess }),
  ...generateMock((sid) => mock.schemaLoad(sid), searchMock(TABLES, tools.jobCancel)),
  ...aboutMockFromUrl(),
  ...extras6Mock(TABLES, params()),
};
Object.assign(mock, clickhouseMock({ ...mock }, (sid) => connections.find((c) => `mock-${c.id}` === sid), (sid) => sessionDb.get(sid) ?? 'gestionale'));
Object.assign(mock, nosql2Mock({ ...mock }, (sid) => connections.find((c) => `mock-${c.id}` === sid), (sid) => sessionDb.get(sid) ?? 'gestionale'));
Object.assign(mock, extras3Mock({ ...mock }, (sid) => connections.find((c) => `mock-${c.id}` === sid)?.target));
Object.assign(mock, b2Mock(params().has('b2'), () => connections, (next) => (connections = next), { ...mock }));
Object.assign(mock, otherdbMock({ ...mock }, (sid) => connections.find((c) => `mock-${c.id}` === sid)));
Object.assign(mock, oracleMock({ ...mock }, (sid) => connections.find((c) => `mock-${c.id}` === sid)));
Object.assign(mock, enterpriseMock({ ...mock }, (sid) => connections.find((c) => `mock-${c.id}` === sid)));
Object.assign(mock, extras8Mock({ ...mock }, (sid) => connections.find((c) => `mock-${c.id}` === sid)));
Object.assign(mock, extras4Mock({ ...mock }, (sid) => connections.find((c) => `mock-${c.id}` === sid.split('@')[0])?.target.driver));
Object.assign(mock, extras5Mock({ ...mock }));
Object.assign(mock, acceptanceMock({ ...mock }));
Object.assign(mock, chaosNet({ ...mock }, params().get('rete')));
Object.assign(mock, chaosCalls({ ...mock }, params().get('caos')));
Object.assign(mock, lazyMock({ ...mock }, params().has('lazy')));
Object.assign(mock, clickhouseGenMock({ ...mock }, (sid) => connections.find((c) => `mock-${c.id}` === sid.split('@')[0]), params().has('chfail')));

if (SHOWCASE) {
  for (const [name, fn] of Object.entries(mock)) {
    if (typeof fn !== 'function') continue;
    (mock as Record<string, unknown>)[name] = (...args: unknown[]) => {
      const wrapped = args.map((a) => (typeof a === 'function' ? (...xs: unknown[]) => (a as (...ys: unknown[]) => unknown)(...xs.map(showcaseValue)) : a));
      const out = (fn as (...ys: unknown[]) => unknown)(...wrapped);
      return out instanceof Promise ? out.then(showcaseValue) : showcaseValue(out);
    };
  }
}

export const backend: Api = SHOWCASE ? mock : englishMock(mock, readAppearance().locale);
