import type { Api, ConnectionProfile, PlanReport } from './api';
import type { Activity_Serialize as Activity, Principals_Serialize as Principals, Report, Sizes_Serialize as Sizes } from './bindings';
import mongoPlan from './mock-plan-mongo.json';
import { DEFAULT_SESSION } from './session';

const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));
const MB = 1024 * 1024;

export const CLUSTER_DEMO: ConnectionProfile[] =
  typeof location !== 'undefined' && new URLSearchParams(location.search).has('cluster')
    ? [
        {
          id: 'redis-cluster',
          name: 'Cache — cluster',
          folder: null,
          environment: 'development',
          color: null,
          session: DEFAULT_SESSION,
          template: { kind: 'none' },
          raw_fields: {},
          target: { driver: 'redis', host: '10.0.6.11', port: 7000, database: 0, user: '', tls: 'disable', ca_file: null, ssh: null, proxy: null, client_cert: null, mode: { kind: 'cluster' } },
        },
      ]
    : [];

const MONGO_USERS: Principals = {
  partial: false,
  quotas: [],
  principals: [
    { name: 'nicedb', host: 'admin', attributes: ['login', 'superuser', 'create_db', 'create_role'], member_of: ['root@admin'] },
    { name: 'report', host: 'gestionale', attributes: ['login'], member_of: ['read@gestionale', 'lettura_ordini@gestionale'] },
    { name: 'magazzino', host: 'gestionale', attributes: ['login'], member_of: ['readWrite@gestionale'] },
    { name: 'lettura_ordini', host: 'gestionale', attributes: [], member_of: ['read@archivio'] },
  ],
  grants: [
    { grantee: 'nicedb@admin', kind: 'database', schema: null, object: 'admin', privilege: 'root', grantable: false },
    { grantee: 'report@gestionale', kind: 'database', schema: null, object: 'gestionale', privilege: 'read', grantable: false },
    { grantee: 'report@gestionale', kind: 'database', schema: null, object: 'gestionale', privilege: 'lettura_ordini', grantable: false },
    { grantee: 'magazzino@gestionale', kind: 'database', schema: null, object: 'gestionale', privilege: 'readWrite', grantable: false },
    { grantee: 'lettura_ordini@gestionale', kind: 'table', schema: 'gestionale', object: 'ordini', privilege: 'find', grantable: false },
    { grantee: 'lettura_ordini@gestionale', kind: 'table', schema: 'gestionale', object: 'ordini', privilege: 'listIndexes', grantable: false },
  ],
};

const CLICKHOUSE_USERS: Principals = {
  partial: false,
  principals: [
    { name: 'nicedb', host: null, attributes: ['login', 'superuser', 'create_db', 'create_role'], member_of: [] },
    { name: 'report', host: null, attributes: ['login'], member_of: ['lettura'] },
    { name: 'etl', host: null, attributes: ['login'], member_of: ['carico'] },
    { name: 'lettura', host: null, attributes: [], member_of: [] },
    { name: 'carico', host: null, attributes: [], member_of: [] },
  ],
  grants: [
    { grantee: 'lettura', kind: 'database', schema: null, object: 'gestionale', privilege: 'SELECT', grantable: false },
    { grantee: 'lettura', kind: 'table', schema: 'archivio', object: 'log_eventi', privilege: 'SELECT', grantable: false },
    { grantee: 'carico', kind: 'table', schema: 'gestionale', object: 'righe_ordine', privilege: 'INSERT', grantable: false },
    { grantee: 'report', kind: 'table', schema: 'gestionale', object: 'clienti', privilege: 'SELECT(ragione_sociale)', grantable: false },
  ],
  quotas: [
    {
      name: 'report_leggero',
      all: false,
      applies_to: ['report', 'lettura'],
      except: [],
      limits: [
        { seconds: 3600, caps: [{ what: 'max_queries', max: 500 }, { what: 'max_read_rows', max: 2_000_000_000 }, { what: 'max_execution_time', max: 600 }] },
        { seconds: 86400, caps: [{ what: 'max_read_bytes', max: 200 * 1024 * MB }] },
      ],
    },
    { name: 'etl_notte', all: false, applies_to: ['etl'], except: [], limits: [{ seconds: 3600, caps: [{ what: 'max_errors', max: 20 }] }] },
  ],
};

const MONGO_ACTIVITY: Activity = {
  partial: false,
  can_cancel: true,
  locks: [],
  sessions: [
    { id: '48211', user: 'nicedb', database: 'gestionale', client: '10.0.4.21:51234', application: 'NiceDB', state: 'active', query: '{"aggregate":"ordini","pipeline":[{"$unwind":"$righe"},{"$group":{"_id":"$righe.articolo","n":{"$sum":1}}}]}', duration_ms: 1840, wait: null, blocked_by: [] },
    { id: '48190', user: 'magazzino', database: 'gestionale', client: '10.0.4.33:40112', application: 'app-magazzino', state: 'active', query: '{"update":"articoli","updates":[{"q":{"codice":"ART-00021"},"u":{"$inc":{"giacenza":-3}}}]}', duration_ms: 12, wait: 'lock', blocked_by: [] },
    { id: '47002', user: 'report', database: 'archivio', client: '10.0.4.40:39001', application: 'report-notturno', state: 'active', query: '{"find":"movimenti","filter":{"anno":2025}}', duration_ms: 46_300, wait: null, blocked_by: [] },
  ],
};

const MONGO_SIZES: Sizes = {
  usage_known: true,
  tables: [
    { namespace: 'gestionale', name: 'ordini', rows: 20_000, data_bytes: 4.1 * MB, index_bytes: 1.3 * MB, total_bytes: 5.4 * MB },
    { namespace: 'gestionale', name: 'clienti', rows: 1200, data_bytes: 0.36 * MB, index_bytes: 0.09 * MB, total_bytes: 0.45 * MB },
    { namespace: 'gestionale', name: 'articoli', rows: 400, data_bytes: 0.08 * MB, index_bytes: 0.03 * MB, total_bytes: 0.11 * MB },
    { namespace: 'gestionale', name: 'agenti', rows: 12, data_bytes: 0.02 * MB, index_bytes: 0.02 * MB, total_bytes: 0.04 * MB },
  ],
  indexes: [
    { namespace: 'gestionale', table: 'ordini', name: '_id_', bytes: 0.5 * MB, scans: 120, unique: true, unused: false },
    { namespace: 'gestionale', table: 'ordini', name: 'cliente._id_1_data_-1', bytes: 0.4 * MB, scans: 3412, unique: false, unused: false },
    { namespace: 'gestionale', table: 'ordini', name: 'numero_1', bytes: 0.3 * MB, scans: 0, unique: false, unused: true },
    { namespace: 'gestionale', table: 'ordini', name: 'stato_1', bytes: 0.1 * MB, scans: 88, unique: false, unused: false },
    { namespace: 'gestionale', table: 'clienti', name: 'partita_iva_1', bytes: 0.05 * MB, scans: 14, unique: true, unused: false },
  ],
};

const SLOTS = [
  ['0–5460', '10.0.6.11:7000', '10.0.6.14:7003'],
  ['5461–10922', '10.0.6.12:7001', '10.0.6.15:7004'],
  ['10923–16383', '10.0.6.13:7002', '10.0.6.16:7005'],
];

export function extras3Mock(base: Api, driver: (sid: string) => ConnectionProfile['target'] | undefined): Partial<Api> {
  const kind = (sid: string) => driver(sid.split('@')[0] ?? sid)?.driver;
  const cluster = (sid: string) => {
    const t = driver(sid.split('@')[0] ?? sid);
    return t?.driver === 'redis' && t.mode.kind === 'cluster';
  };
  return {
    principalsList: async (sid) => {
      const k = kind(sid);
      if (k === 'mongodb') return (await sleep(60), structuredClone(MONGO_USERS));
      if (k === 'clickhouse') return (await sleep(60), structuredClone(CLICKHOUSE_USERS));
      return base.principalsList(sid);
    },
    serverActivity: async (sid) => (kind(sid) === 'mongodb' ? (await sleep(60), structuredClone(MONGO_ACTIVITY)) : base.serverActivity(sid)),
    serverSizes: async (sid) => (kind(sid) === 'mongodb' ? (await sleep(60), structuredClone(MONGO_SIZES)) : base.serverSizes(sid)),
    explain: async (req) => {
      if (kind(req.session_id) !== 'mongodb') return base.explain(req);
      await sleep(200);
      return structuredClone(mongoPlan) as PlanReport;
    },
    serverReport: async (sid) => {
      const report: Report = await base.serverReport(sid);
      if (!cluster(sid)) return report;
      return { sections: [...report.sections, { id: 'cluster_slots', group: 'info', columns: ['slots', 'master', 'replicas'], rows: SLOTS }] };
    },
  };
}
