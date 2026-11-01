import type { Api, ConnectionProfile, Schema, Sizes } from './api';
import type { Capabilities, Edition_Serialize as Edition, Flavor, Relation_Serialize as Relation } from './bindings';
import { choiceOf, isFlavor, type Target } from './drivers';
import { DEFAULT_SESSION } from './session';

const pg = (id: string, name: string, host: string, port: number, database: string, user: string, flavor: Flavor, tls: 'disable' | 'verify' = 'verify'): ConnectionProfile => ({
  id,
  name,
  folder: null,
  environment: id === 'alt-listino' ? 'production' : 'development',
  color: null,
  session: DEFAULT_SESSION,
  template: { kind: 'none' },
  raw_fields: {},
  target: { driver: flavor === 'tidb' || flavor === 'planetscale' ? 'mysql' : 'postgres', host, port, database, user, tls, ca_file: null, ssh: null, proxy: null, client_cert: null, flavor },
});

export const OTHER_DEMO: ConnectionProfile[] = [
  pg('alt-sensori', 'Sensori — serie storiche', '127.0.0.1', 55433, 'gestionale', 'nicedb', 'timescale', 'disable'),
  pg('alt-catalogo', 'Catalogo — cluster', '127.0.0.1', 55434, 'yugabyte', 'yugabyte', 'yugabyte', 'disable'),
  pg('alt-vetrina', 'Vetrina — anteprima', 'ep-quiet-meadow-482913-pooler.eu-central-1.aws.neon.tech', 5432, 'neondb', 'neondb_owner', 'neon'),
  pg('alt-clienti', 'App clienti', 'db.qbvnrkxtmwdzpelfhsoa.supabase.co', 5432, 'postgres', 'postgres', 'supabase'),
  pg('alt-ordini', 'Ordini — analisi', '127.0.0.1', 54000, 'test', 'root', 'tidb', 'disable'),
  pg('alt-listino', 'Listino — produzione', 'aws.connect.psdb.cloud', 3306, 'listino', 'r7x2kq9w', 'planetscale'),
];

const EDITIONS: Record<Flavor, Edition & { product: string }> = {
  timescale: { flavor: 'timescale', version: '2.31.0', facts: [{ kind: 'base', value: 'PostgreSQL 17.11' }, { kind: 'extension', value: { name: 'postgis', version: '3.5.2' } }], limits: ['hypertable_dump'], product: 'TimescaleDB' },
  yugabyte: { flavor: 'yugabyte', version: '2024.2.0.0', facts: [{ kind: 'base', value: 'PostgreSQL 11.2' }, { kind: 'nodes', value: 3 }], limits: ['distributed_locks', 'own_dump_tool'], product: 'YugabyteDB' },
  neon: {
    flavor: 'neon',
    version: null,
    facts: [{ kind: 'base', value: 'PostgreSQL 17.4' }, { kind: 'endpoint', value: 'ep-quiet-meadow-482913' }, { kind: 'region', value: 'eu-central-1' }, { kind: 'pooled', value: 'transaction' }, { kind: 'branch', value: 'br-wild-sun-a1b2c3' }],
    limits: ['pooled_session', 'scale_to_zero'],
    product: 'Neon',
  },
  supabase: { flavor: 'supabase', version: null, facts: [{ kind: 'base', value: 'PostgreSQL 15.8' }, { kind: 'project', value: 'qbvnrkxtmwdzpelfhsoa' }, { kind: 'extension', value: { name: 'pg_graphql', version: '1.5.11' } }], limits: ['managed_schemas'], product: 'Supabase' },
  tidb: { flavor: 'tidb', version: 'v8.5.1', facts: [{ kind: 'base', value: 'MySQL 8.0.11' }, { kind: 'nodes', value: 4 }, { kind: 'store', value: 'tikv' }], limits: ['distributed_locks', 'own_dump_tool', 'own_plans'], product: 'TiDB' },
  planetscale: { flavor: 'planetscale', version: null, facts: [{ kind: 'base', value: 'MySQL 8.0.34' }, { kind: 'store', value: 'listino@primary' }], limits: ['foreign_keys_optional', 'deploy_requests', 'no_routines', 'dashboard_users'], product: 'PlanetScale' },
};

const flavorOfTarget = (t: Target | undefined): Flavor | null => {
  if (!t) return null;
  const c = choiceOf(t);
  return isFlavor(c) ? c : null;
};

const shown = (f: Flavor) => EDITIONS[f].version ?? EDITIONS[f].facts.find((x) => x.kind === 'base')?.value ?? '';

export function editionOf(t: Target | undefined): Edition | null {
  const f = flavorOfTarget(t);
  if (!f) return null;
  const { product: _p, ...edition } = EDITIONS[f];
  return structuredClone(edition);
}

function capsFor(f: Flavor, caps: Capabilities): Capabilities {
  const next = structuredClone(caps);
  if (f === 'timescale') next.tree.groups = ['tables', 'views', 'materialized_views', 'functions', 'sequences', 'types', 'triggers'];
  if (f === 'tidb') next.tree.groups = ['tables', 'views', 'sequences'];
  if (f === 'planetscale') {
    next.tree.groups = ['tables', 'views'];
    next.tools = next.tools.filter((t) => t !== 'users' && t !== 'backup' && t !== 'seclab');
  }
  return next;
}

const col = (name: string, type_name: string, primary_key = false, nullable = !primary_key) => ({ name, type_name, nullable, primary_key });
const rel = (name: string, rows: number | null, columns: ReturnType<typeof col>[]): Relation => ({ name, estimated_rows: rows, columns, indexes: [], foreign_keys: [] });
const ns = (name: string, extra: Partial<Schema['namespaces'][number]> = {}): Schema['namespaces'][number] => ({ name, tables: [], views: [], functions: [], sequences: [], types: [], triggers: [], ...extra });

function timescaleSchema(): Schema {
  const misure = rel('misure', 48_210_554, [col('istante', 'timestamptz', false, false), col('sensore_id', 'int4', false, false), col('temperatura', 'float8'), col('umidita', 'float8')]);
  return {
    databases: ['gestionale', 'postgres'],
    current_database: 'gestionale',
    namespaces: [
      ns('public', {
        tables: [rel('sensori', 412, [col('id', 'int4', true), col('nome', 'text'), col('reparto', 'text')]), misure, rel('allarmi', 18_240, [col('istante', 'timestamptz', false, false), col('sensore_id', 'int4'), col('livello', 'text')])],
        views: [rel('ultimi_valori', null, [col('sensore_id', 'int4'), col('istante', 'timestamptz'), col('temperatura', 'float8')])],
        materialized_views: [rel('misure_orarie', 1_204_880, [col('ora', 'timestamptz'), col('sensore_id', 'int4'), col('media', 'float8')]), rel('misure_giornaliere', 50_210, [col('giorno', 'timestamptz'), col('sensore_id', 'int4'), col('media', 'float8')])],
        tags: {
          misure: [{ kind: 'hypertable', chunks: 104, compressed: 96 }],
          allarmi: [{ kind: 'hypertable', chunks: 12, compressed: 0 }],
          misure_orarie: [{ kind: 'continuous_aggregate' }],
          misure_giornaliere: [{ kind: 'continuous_aggregate' }],
        },
      }),
      ns('timescaledb_information', { managed: 'timescale', views: ['chunks', 'continuous_aggregates', 'dimensions', 'hypertables', 'jobs'].map((v) => rel(v, null, [col('hypertable_name', 'name')])) }),
      ns('timescaledb_experimental', { managed: 'timescale', views: [rel('policies', null, [col('relation_name', 'name')])] }),
    ],
  };
}

function supabaseSchema(): Schema {
  const managed = (name: string, tables: string[]) => ns(name, { managed: 'supabase', tables: tables.map((t) => rel(t, null, [col('id', 'uuid', true)])) });
  return {
    databases: ['postgres'],
    current_database: 'postgres',
    namespaces: [
      ns('public', {
        tables: [
          rel('profili', 2_841, [col('id', 'uuid', true), col('nome', 'text'), col('avatar_url', 'text')]),
          rel('ordini', 12_940, [col('id', 'int8', true), col('cliente', 'uuid'), col('totale', 'numeric')]),
          rel('listini', 38, [col('id', 'int4', true), col('nome', 'text')]),
        ],
        tags: { profili: [{ kind: 'rls', policies: 2 }], ordini: [{ kind: 'rls', policies: 3 }] },
      }),
      managed('auth', ['users', 'identities', 'sessions', 'refresh_tokens', 'mfa_factors']),
      managed('storage', ['buckets', 'objects', 'migrations']),
      managed('realtime', ['messages', 'subscription']),
      managed('extensions', []),
      managed('vault', ['secrets']),
      managed('graphql_public', []),
    ],
  };
}

const MB = 1024 * 1024;

function timescaleSizes(): Sizes {
  const chunk = (from: string, to: string, bytes: number, compressed: boolean) => ({ namespace: 'public', table: 'misure', partition: `${from} → ${to}`, parts: null, rows: null, bytes, compressed });
  return {
    tables: [
      { namespace: 'public', name: 'misure', rows: 48_210_554, data_bytes: null, index_bytes: null, total_bytes: 1_212 * MB },
      { namespace: 'public', name: 'allarmi', rows: 18_240, data_bytes: null, index_bytes: null, total_bytes: 6.2 * MB },
      { namespace: 'public', name: 'sensori', rows: 412, data_bytes: 0.06 * MB, index_bytes: 0.03 * MB, total_bytes: 0.09 * MB },
    ],
    indexes: [
      { namespace: 'public', table: 'misure', name: 'misure_istante_idx', bytes: 210 * MB, scans: 18_204, unique: false, unused: false },
      { namespace: 'public', table: 'sensori', name: 'sensori_pkey', bytes: 0.02 * MB, scans: 120_440, unique: true, unused: false },
    ],
    usage_known: true,
    hypertables: [
      { namespace: 'public', name: 'misure', chunks: 104, compressed_chunks: 96, before_compression: 9_840 * MB, after_compression: 912 * MB, total_bytes: 1_212 * MB },
      { namespace: 'public', name: 'allarmi', chunks: 12, compressed_chunks: 0, before_compression: null, after_compression: null, total_bytes: 6.2 * MB },
    ],
    partitions: [
      chunk('2026-10-29', '2026-11-05', 74 * MB, false),
      chunk('2026-10-22', '2026-10-29', 102 * MB, false),
      chunk('2026-10-15', '2026-10-22', 9.6 * MB, true),
      chunk('2026-10-08', '2026-10-15', 9.4 * MB, true),
      chunk('2026-10-01', '2026-10-08', 9.5 * MB, true),
    ],
  };
}

export function otherdbMock(base: Api, profile: (sessionId: string) => ConnectionProfile | undefined): Partial<Api> {
  const flavor = (sid: string) => flavorOfTarget(profile(sid.split('@')[0] ?? sid)?.target);
  return {
    connectionTest: async (p, secrets) => {
      const info = await base.connectionTest(p, secrets);
      const f = flavorOfTarget(p.target);
      return f ? { ...info, product: EDITIONS[f].product, version: shown(f) } : info;
    },
    sessionOpen: async (id, db) => {
      const info = await base.sessionOpen(id, db);
      const f = flavorOfTarget(profile(`mock-${id}`)?.target);
      if (!f) return info;
      return { ...info, caps: capsFor(f, info.caps), server: { ...info.server, product: EDITIONS[f].product, version: shown(f) }, edition: editionOf(profile(`mock-${id}`)?.target) };
    },
    schemaLoad: async (sid) => {
      const f = flavor(sid);
      if (f === 'timescale') return timescaleSchema();
      if (f === 'supabase') return supabaseSchema();
      return base.schemaLoad(sid);
    },
    serverSizes: async (sid) => (flavor(sid) === 'timescale' ? timescaleSizes() : base.serverSizes(sid)),
  };
}
