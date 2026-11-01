import {
  siApachecassandra,
  siClickhouse,
  siCockroachlabs,
  siDuckdb,
  siElasticsearch,
  siMariadb,
  siMongodb,
  siMysql,
  siNeon,
  siPlanetscale,
  siPostgresql,
  siRedis,
  siSap,
  siSqlite,
  siSupabase,
  siTidb,
  siTimescale,
  siTurso,
} from 'simple-icons';
import * as marks from './engine-marks';
import type { ConnectionProfile } from './api';
import type { Dialect, Flavor, TlsMode } from './bindings';

export type DriverKind = 'relational' | 'document' | 'keyvalue' | 'analytical' | 'widecolumn' | 'search';
export type DriverWhere = 'server' | 'file' | 'cloud';

export interface Driver {
  id: string;
  name: string;
  kind: DriverKind;
  where: DriverWhere;
  mark: { path: string; view?: string } | { monogram: string };
  base?: 'postgres' | 'mysql';
}

export const DRIVERS: readonly Driver[] = [
  { id: 'postgres', name: 'PostgreSQL', kind: 'relational', where: 'server', mark: { path: siPostgresql.path } },
  { id: 'mysql', name: 'MySQL / MariaDB', kind: 'relational', where: 'server', mark: { path: siMysql.path } },
  { id: 'mariadb', name: 'MariaDB', kind: 'relational', where: 'server', mark: { path: siMariadb.path } },
  { id: 'sqlserver', name: 'SQL Server', kind: 'relational', where: 'server', mark: { path: marks.sqlserver, view: '0 0 128 128' } },
  { id: 'oracle', name: 'Oracle', kind: 'relational', where: 'server', mark: { path: marks.oracle, view: '0.25 50.55 27 27' } },
  { id: 'firebird', name: 'Firebird', kind: 'relational', where: 'server', mark: { path: marks.firebird, view: '0 0 128 128' } },
  { id: 'db2', name: 'Db2', kind: 'relational', where: 'server', mark: { path: marks.ibm, view: '0 -299.4 1000 1000' } },
  { id: 'hana', name: 'SAP HANA', kind: 'relational', where: 'server', mark: { path: siSap.path } },
  { id: 'odbc', name: 'ODBC', kind: 'relational', where: 'server', mark: { path: marks.odbc } },
  { id: 'cockroach', name: 'CockroachDB', kind: 'relational', where: 'server', mark: { path: siCockroachlabs.path } },
  { id: 'sqlite', name: 'SQLite', kind: 'relational', where: 'file', mark: { path: siSqlite.path } },
  { id: 'libsql', name: 'libSQL / Turso', kind: 'relational', where: 'server', mark: { path: siTurso.path } },
  { id: 'duckdb', name: 'DuckDB', kind: 'analytical', where: 'file', mark: { path: siDuckdb.path } },
  { id: 'clickhouse', name: 'ClickHouse', kind: 'analytical', where: 'server', mark: { path: siClickhouse.path } },
  { id: 'mongodb', name: 'MongoDB', kind: 'document', where: 'server', mark: { path: siMongodb.path } },
  { id: 'redis', name: 'Redis / Valkey', kind: 'keyvalue', where: 'server', mark: { path: siRedis.path } },
  { id: 'timescale', name: 'TimescaleDB', kind: 'analytical', where: 'server', mark: { path: siTimescale.path }, base: 'postgres' },
  { id: 'yugabyte', name: 'YugabyteDB', kind: 'relational', where: 'server', mark: { path: marks.yugabyte, view: '0 0 128 128' }, base: 'postgres' },
  { id: 'neon', name: 'Neon', kind: 'relational', where: 'cloud', mark: { path: siNeon.path }, base: 'postgres' },
  { id: 'supabase', name: 'Supabase', kind: 'relational', where: 'cloud', mark: { path: siSupabase.path }, base: 'postgres' },
  { id: 'tidb', name: 'TiDB', kind: 'relational', where: 'server', mark: { path: siTidb.path }, base: 'mysql' },
  { id: 'planetscale', name: 'PlanetScale', kind: 'relational', where: 'cloud', mark: { path: siPlanetscale.path }, base: 'mysql' },
  { id: 'cassandra', name: 'Cassandra', kind: 'widecolumn', where: 'server', mark: { path: siApachecassandra.path } },
  { id: 'dynamodb', name: 'DynamoDB', kind: 'widecolumn', where: 'server', mark: { path: marks.dynamodb, view: '0 0 128 128' } },
  { id: 'elasticsearch', name: 'Elasticsearch', kind: 'search', where: 'server', mark: { path: siElasticsearch.path } },
];

export type Target = ConnectionProfile['target'];
export type DriverId = Target['driver'];
export type NetworkTarget = Extract<Target, { host: string }>;
export type NetworkDriver = NetworkTarget['driver'];

export const SUPPORTED: readonly DriverId[] = ['postgres', 'mysql', 'sqlserver', 'oracle', 'db2', 'hana', 'firebird', 'cockroach', 'sqlite', 'duckdb', 'libsql', 'clickhouse', 'mongodb', 'redis', 'cassandra', 'dynamodb', 'elasticsearch', 'odbc'];

export type ChoiceId = DriverId | Flavor;
export const CHOICES: readonly ChoiceId[] = ['postgres', 'timescale', 'yugabyte', 'neon', 'supabase', 'mysql', 'tidb', 'planetscale', 'sqlserver', 'oracle', 'db2', 'hana', 'firebird', 'cockroach', 'sqlite', 'duckdb', 'libsql', 'clickhouse', 'mongodb', 'redis', 'cassandra', 'dynamodb', 'elasticsearch', 'odbc'];

const FLAVORS: readonly Flavor[] = ['timescale', 'neon', 'supabase', 'yugabyte', 'tidb', 'planetscale'];
export const isFlavor = (id: string): id is Flavor => (FLAVORS as readonly string[]).includes(id);

export function splitChoice(id: ChoiceId): { driver: DriverId; flavor: Flavor | null } {
  if (!isFlavor(id)) return { driver: id, flavor: null };
  return { driver: id === 'tidb' || id === 'planetscale' ? 'mysql' : 'postgres', flavor: id };
}

export const choiceOf = (t: Target): ChoiceId => ((t.driver === 'postgres' || t.driver === 'mysql') && t.flavor ? t.flavor : t.driver);

export function driverOf(t: Target, seen?: Flavor | null): Driver {
  const id = seen ?? choiceOf(t);
  return DRIVERS.find((d) => d.id === id) ?? DRIVERS.find((d) => d.id === t.driver) ?? DRIVERS[0]!;
}

export const CLOUD: readonly Flavor[] = ['neon', 'supabase', 'planetscale'];
export const needsTls = (id: ChoiceId): boolean => isFlavor(id) && CLOUD.includes(id);

export const hasTls = (d: DriverId): boolean => isNetworkDriver(d) && d !== 'firebird';

export function isLocalHost(host: string): boolean {
  const h = host.trim().toLowerCase().replace(/^\[(.*)\]$/, '$1');
  return h === 'localhost' || h.endsWith('.localhost') || /^127(\.\d{1,3}){3}$/.test(h) || h === '::1' || h.startsWith('/');
}

export function defaultTls(id: ChoiceId, host: string): TlsMode {
  if (!hasTls(splitChoice(id).driver)) return 'disable';
  return isLocalHost(host) && !needsTls(id) ? 'disable' : 'verify';
}

export interface Preset {
  port: number;
  host: string;
  database: string;
  user: string;
}

export const PRESETS: Record<Flavor, Preset> = {
  timescale: { port: 5432, host: 'localhost', database: 'postgres', user: 'postgres' },
  yugabyte: { port: 5433, host: 'localhost', database: 'yugabyte', user: 'yugabyte' },
  neon: { port: 5432, host: 'ep-name-123456.eu-central-1.aws.neon.tech', database: 'neondb', user: 'neondb_owner' },
  supabase: { port: 5432, host: 'db.abcdefghijklmnop.supabase.co', database: 'postgres', user: 'postgres' },
  tidb: { port: 4000, host: 'localhost', database: 'test', user: 'root' },
  planetscale: { port: 3306, host: 'aws.connect.psdb.cloud', database: '', user: '' },
};

export const DEFAULT_PORT: Record<NetworkDriver, number> = { postgres: 5432, mysql: 3306, sqlserver: 1433, cockroach: 26257, mongodb: 27017, clickhouse: 8123, redis: 6379, firebird: 3050, oracle: 1521, db2: 50000, hana: 30015, cassandra: 9042, elasticsearch: 9200 };

export const defaultPort = (d: NetworkDriver, tls: TlsMode): number => (d === 'clickhouse' && tls !== 'disable' ? 8443 : DEFAULT_PORT[d]);

export const takesClientCert = (d: DriverId): boolean => isNetworkDriver(d) && d !== 'firebird' && d !== 'oracle' && d !== 'db2' && d !== 'hana';

export const isNetworkDriver = (d: DriverId): d is NetworkDriver => d in DEFAULT_PORT;

export const isNetwork = (t: Target): t is NetworkTarget => isNetworkDriver(t.driver);

export function dialectOf(driver: DriverId): Dialect | null {
  switch (driver) {
    case 'mongodb':
    case 'redis':
    case 'cassandra':
    case 'dynamodb':
    case 'elasticsearch':
      return null;
    case 'sqlite':
    case 'libsql':
      return 'sqlite';
    case 'mysql':
      return 'mysql';
    case 'sqlserver':
      return 'mssql';
    case 'duckdb':
      return 'duckdb';
    case 'clickhouse':
      return 'clickhouse';
    case 'firebird':
      return 'firebird';
    case 'oracle':
      return 'oracle';
    case 'db2':
      return 'db2';
    case 'hana':
      return 'hana';
    case 'odbc':
      return 'ansi';
    case 'postgres':
    case 'cockroach':
      return 'postgres';
  }
}

export function whereOf(t: Target): string {
  if (t.driver === 'mongodb' && t.srv) return `${t.host}/${t.database}`;
  if (t.driver === 'redis') return `${t.host}:${t.port}/db${t.database}`;
  if (t.driver === 'oracle' && t.connect_string.trim()) return t.connect_string.trim();
  if (t.driver === 'dynamodb') return t.endpoint.replace(/^https?:\/\//, '') || `dynamodb.${t.region}`;
  if (t.driver === 'elasticsearch' && !t.database) return `${t.host}:${t.port}`;
  if (t.driver === 'hana' && !t.database) return `${t.host}:${t.port}`;
  if (isNetwork(t)) return `${t.host}:${t.port}/${t.database}`;
  if (t.driver === 'odbc') return t.dsn ? `DSN ${t.dsn}` : t.driver_path.split(/[\\/]/).pop() || 'ODBC';
  if (t.driver === 'libsql' && t.location === 'remote') return t.url;
  return t.path || ':memory:';
}

export function databaseOf(t: Target): string {
  if (t.driver === 'redis') return `db${t.database}`;
  if (t.driver === 'oracle' && t.connect_string.trim()) return /SERVICE_NAME\s*=\s*([^)\s]+)/i.exec(t.connect_string)?.[1] ?? t.connect_string.trim().split('/').pop()?.split(/[?:]/)[0] ?? '';
  if (t.driver === 'dynamodb') return t.region;
  if (t.driver === 'elasticsearch') return t.database || t.host;
  if (t.driver === 'hana') return t.database || t.host;
  if (isNetwork(t)) return t.database;
  if (t.driver === 'odbc') return t.dsn || /(?:^|;)\s*(?:database|dbq|dbname)\s*=\s*([^;]+)/i.exec(t.connect_string)?.[1]?.trim() || 'ODBC';
  if (t.driver === 'libsql' && t.location === 'remote') return t.url.replace(/^[a-z]+:\/\//, '').split(/[./?]/)[0] ?? '';
  return t.path.split(/[\\/]/).pop() || 'memory';
}
