import type { ConnectionProfile } from './api';
import type { TlsMode } from './bindings';
import { isFlavor, PRESETS, splitChoice, type ChoiceId } from './drivers';

const SSL = { disable: 'disable', verify: 'verify-full', insecure: 'require' } as const;

export function connectionUrl(p: ConnectionProfile): string {
  const t = p.target;
  if (t.driver === 'sqlite') return `sqlite://${encodeURI(t.path)}${t.read_only ? '?mode=ro' : ''}`;
  if (t.driver === 'duckdb') return `duckdb://${t.path ? encodeURI(t.path) : ':memory:'}${t.read_only ? '?access_mode=read_only' : ''}`;
  if (t.driver === 'libsql') return t.location === 'remote' ? t.url : `file:${encodeURI(t.path)}${t.read_only ? '?mode=ro' : ''}`;
  if (t.driver === 'dynamodb') return t.endpoint || `https://dynamodb.${t.region}.amazonaws.com`;
  if (t.driver === 'odbc') return t.dsn ? `odbc:DSN=${t.dsn}` : `odbc:${[t.driver_path && `DRIVER=${t.driver_path}`, t.connect_string].filter(Boolean).join(';')}`;
  const user = t.user ? `${encodeURIComponent(t.user)}@` : '';
  if (t.driver === 'oracle' && t.connect_string.trim()) {
    const cs = t.connect_string.trim();
    return cs.startsWith('(') ? cs : `oracle://${user}${cs}`;
  }
  const host = t.host.includes(':') ? `[${t.host}]` : t.host;
  if (t.driver === 'redis') return `${t.tls === 'disable' ? 'redis' : 'rediss'}://${user}${host}:${t.port}/${t.database}`;
  const db = encodeURIComponent(t.database);
  if (t.driver === 'postgres' || t.driver === 'cockroach') return `postgresql://${user}${host}:${t.port}/${db}?sslmode=${SSL[t.tls]}`;
  if (t.driver === 'mongodb') {
    const query = [t.auth_source && `authSource=${encodeURIComponent(t.auth_source)}`, t.tls !== 'disable' && 'tls=true', t.tls === 'insecure' && 'tlsInsecure=true', t.options].filter(Boolean).join('&');
    return `${t.srv ? 'mongodb+srv' : 'mongodb'}://${user}${host}${t.srv ? '' : `:${t.port}`}/${db}${query ? `?${query}` : ''}`;
  }
  if (t.driver === 'cassandra') return `cassandra://${user}${host}:${t.port}/${db}${t.tls === 'disable' ? '' : '?ssl=true'}`;
  if (t.driver === 'elasticsearch') return `${t.tls === 'disable' ? 'http' : 'https'}://${user}${host}:${t.port}/${db}`;
  if (t.driver === 'clickhouse') return `${t.tls === 'disable' ? 'http' : 'https'}://${user}${host}:${t.port}/?database=${db}`;
  if (t.driver === 'firebird') return `firebird://${user}${host}:${t.port}/${db}`;
  if (t.driver === 'db2') return `db2://${user}${host}:${t.port}/${db}${t.tls === 'disable' ? '' : '?security=ssl'}`;
  if (t.driver === 'hana') return `${t.tls === 'disable' ? 'hdbsql' : 'hdbsqls'}://${user}${host}:${t.port}${t.database ? `?db=${encodeURIComponent(t.database)}` : ''}`;
  if (t.driver === 'oracle') {
    const query = [t.sid && 'sid=true', t.tls !== 'disable' && 'protocol=tcps'].filter(Boolean).join('&');
    return `oracle://${user}${host}:${t.port}/${db}${query ? `?${query}` : ''}`;
  }
  if (t.driver === 'sqlserver') return `sqlserver://${user}${host}:${t.port}?database=${db}&encrypt=${t.tls !== 'disable'}&trustServerCertificate=${t.tls === 'insecure'}`;
  return `mysql://${user}${host}:${t.port}/${db}${t.tls === 'disable' ? '' : `?ssl-mode=${t.tls === 'verify' ? 'VERIFY_IDENTITY' : 'REQUIRED'}`}`;
}

export interface ParsedUrl {
  choice: ChoiceId;
  host: string;
  port: number;
  database: string;
  user: string;
  password: string;
  tls: TlsMode;
}

const HOSTED: [RegExp, ChoiceId][] = [
  [/\.neon\.tech$/, 'neon'],
  [/\.supabase\.(co|com)$/, 'supabase'],
  [/\.psdb\.cloud$/, 'planetscale'],
  [/\.tidbcloud\.com$/, 'tidb'],
];

function tlsOf(q: URLSearchParams, hosted: boolean): TlsMode {
  const mode = (q.get('sslmode') ?? q.get('ssl-mode') ?? q.get('sslMode') ?? '').toLowerCase();
  if (mode === 'disable' || mode === 'disabled') return 'disable';
  if (mode === 'require' || mode === 'required' || mode === 'prefer' || mode === 'preferred') return hosted ? 'verify' : 'insecure';
  if (mode.startsWith('verify')) return 'verify';
  if (q.get('ssl') === 'true' || q.get('sslaccept') === 'strict') return 'verify';
  return hosted ? 'verify' : 'disable';
}

export function parseConnectionUrl(text: string, current?: ChoiceId): ParsedUrl | null {
  let u: URL;
  try {
    u = new URL(text.trim());
  } catch {
    return null;
  }
  const scheme = u.protocol.replace(/:$/, '').toLowerCase();
  const family = scheme === 'postgres' || scheme === 'postgresql' || scheme === 'cockroachdb' ? 'postgres' : scheme === 'mysql' || scheme === 'mariadb' ? 'mysql' : scheme === 'oracle' ? 'oracle' : null;
  if (!family || !u.hostname) return null;
  const host = u.hostname.replace(/^\[|\]$/g, '');
  const cloud = /\.cockroachlabs\.cloud$/.test(host.toLowerCase());
  const hosted = HOSTED.find(([re]) => re.test(host.toLowerCase()))?.[1];
  const fits = (c: ChoiceId | undefined): c is ChoiceId => !!c && (c === 'cockroach' ? family === 'postgres' : splitChoice(c).driver === family);
  const choice: ChoiceId =
    scheme === 'cockroachdb' || cloud ? 'cockroach' : hosted && splitChoice(hosted).driver === family ? hosted : fits(current) && !(current && HOSTED.some(([, h]) => h === current)) ? current : family;
  const preset = isFlavor(choice) ? PRESETS[choice].port : choice === 'cockroach' ? 26257 : family === 'postgres' ? 5432 : family === 'oracle' ? 1521 : 3306;
  const decode = (s: string) => {
    try {
      return decodeURIComponent(s);
    } catch {
      return s;
    }
  };
  return {
    choice,
    host,
    port: u.port ? Number(u.port) : preset,
    database: decode(u.pathname.replace(/^\//, '')),
    user: decode(u.username),
    password: decode(u.password),
    tls: family === 'oracle' ? (u.searchParams.get('protocol')?.toLowerCase() === 'tcps' ? 'verify' : 'disable') : tlsOf(u.searchParams, !!hosted || cloud),
  };
}
