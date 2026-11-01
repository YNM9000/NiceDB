import type { ConnectionProfile } from './api';
import type { Consistency, Environment, Flavor, OracleRole, ProxyKind, RedisMode, SshAuth, TlsMode } from './bindings';
import { DEFAULT_PORT, isNetwork, isNetworkDriver, type DriverId, type Target } from './drivers';
import { DEFAULT_SESSION, wholeNumber } from './session';
import { hasVars } from './vars';

export const MAX_JUMPS = 8;

export interface HopForm {
  host: string;
  port: string;
  user: string;
  method: SshAuth['method'];
  key: string;
  secret: string;
  saved: number | null;
}

export interface ProfileForm {
  name: string;
  folder: string;
  environment: Environment;
  driver: DriverId;
  flavor: Flavor | null;
  role: 'none' | 'model' | 'follows';
  modelId: string;
  host: string;
  port: string;
  database: string;
  user: string;
  tls: TlsMode;
  caFile: string;
  useCert: boolean;
  certFile: string;
  keyFile: string;
  useSsh: boolean;
  ssh: HopForm;
  jumps: HopForm[];
  useProxy: boolean;
  proxyKind: ProxyKind;
  proxyHost: string;
  proxyPort: string;
  proxyUser: string;
  path: string;
  readOnly: boolean;
  location: 'file' | 'remote';
  url: string;
  init: string;
  keepalive: number;
  reconnect: boolean;
  idle: number;
  rowLimit: string;
  srv: boolean;
  authSource: string;
  options: string;
  redisMode: 'single' | 'cluster' | 'sentinel';
  sentinelMaster: string;
  oraAddress: 'host' | 'string';
  oraSid: boolean;
  connectString: string;
  oraRole: OracleRole;
  odbcDsn: string;
  odbcDriver: string;
  consistency: Consistency;
  datacenter: string;
  region: string;
  endpoint: string;
  awsProfile: string;
}

export const FIELD_KEYS: Record<string, (keyof ProfileForm)[]> = {
  host: ['host'],
  port: ['port'],
  database: ['database'],
  user: ['user'],
  tls: ['tls'],
  ca_file: ['caFile'],
  client_cert: ['useCert', 'certFile', 'keyFile'],
  ssh: ['useSsh', 'ssh', 'jumps'],
  proxy: ['useProxy', 'proxyKind', 'proxyHost', 'proxyPort', 'proxyUser'],
  path: ['path'],
  read_only: ['readOnly'],
  url: ['url'],
  location: ['location'],
  'session.init': ['init'],
  'session.keepalive_secs': ['keepalive'],
  'session.reconnect': ['reconnect'],
  'session.idle_minutes': ['idle'],
  'session.row_limit': ['rowLimit'],
  srv: ['srv'],
  auth_source: ['authSource'],
  options: ['options'],
  mode: ['redisMode', 'sentinelMaster'],
  sid: ['oraSid'],
  connect_string: ['oraAddress', 'connectString'],
  role: ['oraRole'],
  dsn: ['odbcDsn'],
  driver_path: ['odbcDriver'],
  consistency: ['consistency'],
  datacenter: ['datacenter'],
  region: ['region'],
  endpoint: ['endpoint'],
  profile: ['awsProfile'],
};

export const blankHop = (): HopForm => ({ host: '', port: '22', user: '', method: 'agent', key: '~/.ssh/id_ed25519', secret: '', saved: null });

const text = (raw: Record<string, string>, path: string, n: number | undefined) => raw[path] ?? (n === undefined ? '' : String(n));

export function formOf(p: ConnectionProfile | null): ProfileForm {
  const t = p?.target;
  const net = t && isNetwork(t) ? t : null;
  const cert = net && 'client_cert' in net ? net.client_cert : null;
  const mongo = t?.driver === 'mongodb' ? t : null;
  const redisMode = t?.driver === 'redis' ? t.mode : undefined;
  const ora = t?.driver === 'oracle' ? t : null;
  const cql = t?.driver === 'cassandra' ? t : null;
  const ddb = t?.driver === 'dynamodb' ? t : null;
  const odbc = t?.driver === 'odbc' ? t : null;
  const file = t && 'path' in t ? t : null;
  const remote = t?.driver === 'libsql' && t.location === 'remote' ? t : null;
  const proxy = net?.proxy ?? remote?.proxy ?? null;
  const raw = p?.raw_fields ?? {};
  const s = p?.session ?? DEFAULT_SESSION;
  const hop = (h: { host: string; port: number; user: string; auth: SshAuth }, path: string, saved: number | null = null): HopForm => ({
    host: h.host,
    port: text(raw, path, h.port),
    user: h.user,
    method: h.auth.method,
    key: h.auth.method === 'key' ? h.auth.path : '~/.ssh/id_ed25519',
    secret: '',
    saved,
  });
  const tpl = p?.template ?? { kind: 'none' as const };
  return {
    name: p?.name ?? '',
    folder: p?.folder ?? '',
    environment: p?.environment ?? 'development',
    driver: t?.driver ?? 'postgres',
    flavor: (t?.driver === 'postgres' || t?.driver === 'mysql') && t.flavor ? t.flavor : null,
    role: tpl.kind,
    modelId: tpl.kind === 'follows' ? tpl.id : '',
    host: net?.host ?? 'localhost',
    port: text(raw, 'port', net?.port ?? 5432),
    database: typeof net?.database === 'number' ? text(raw, 'database', net.database) : (net?.database ?? ''),
    user: net?.user ?? ddb?.user ?? odbc?.user ?? '',
    tls: net && 'tls' in net ? net.tls : 'disable',
    caFile: (net && 'ca_file' in net ? net.ca_file : null) ?? '',
    useCert: !!cert,
    certFile: cert?.cert ?? '',
    keyFile: cert?.key ?? '',
    useSsh: !!net?.ssh,
    ssh: net?.ssh ? hop(net.ssh, 'ssh.port') : blankHop(),
    jumps: (net?.ssh?.jumps ?? []).map((j, i) => hop(j, `ssh.jumps.${i}.port`, i)),
    useProxy: !!proxy,
    proxyKind: proxy?.kind ?? 'socks5',
    proxyHost: proxy?.host ?? '',
    proxyPort: proxy ? text(raw, 'proxy.port', proxy.port) : '1080',
    proxyUser: proxy?.user ?? '',
    path: file?.path ?? '',
    readOnly: file?.read_only ?? false,
    location: remote ? 'remote' : 'file',
    url: remote?.url ?? '',
    init: s.init,
    keepalive: s.keepalive_secs,
    reconnect: s.reconnect,
    idle: s.idle_minutes,
    rowLimit: s.row_limit === null ? '' : String(s.row_limit),
    srv: mongo?.srv ?? false,
    authSource: mongo?.auth_source ?? '',
    options: mongo?.options ?? '',
    redisMode: redisMode?.kind ?? 'single',
    sentinelMaster: redisMode?.kind === 'sentinel' ? redisMode.master : '',
    oraAddress: ora?.connect_string.trim() ? 'string' : 'host',
    oraSid: ora?.sid ?? false,
    connectString: ora?.connect_string ?? odbc?.connect_string ?? '',
    oraRole: ora?.role ?? 'normal',
    odbcDsn: odbc?.dsn ?? '',
    odbcDriver: odbc?.driver_path ?? '',
    consistency: cql?.consistency ?? 'local_quorum',
    datacenter: cql?.datacenter ?? '',
    region: ddb?.region ?? 'eu-west-1',
    endpoint: ddb?.endpoint ?? '',
    awsProfile: ddb?.profile ?? '',
  };
}

export const portOk = (v: string) => (hasVars(v) ? true : /^\d+$/.test(v.trim()) && +v >= 1 && +v <= 65535);

function port(v: string, path: string, raw: Record<string, string>, fallback: number): number {
  if (hasVars(v)) {
    raw[path] = v.trim();
    return 0;
  }
  return +v || fallback;
}

export function targetOf(f: ProfileForm): { target: Target; raw: Record<string, string> } {
  const raw: Record<string, string> = {};
  const auth = (h: HopForm): SshAuth => (h.method === 'key' ? { method: 'key', path: h.key.trim() } : { method: h.method });
  const local = { path: f.path.trim(), read_only: f.readOnly };
  const d = f.driver;
  if (isNetworkDriver(d)) {
    const ssh = f.useSsh
      ? {
          host: f.ssh.host.trim(),
          port: port(f.ssh.port, 'ssh.port', raw, 22),
          user: f.ssh.user.trim(),
          auth: auth(f.ssh),
          jumps: f.jumps.slice(0, MAX_JUMPS).map((j, i) => ({ host: j.host.trim(), port: port(j.port, `ssh.jumps.${i}.port`, raw, 22), user: j.user.trim(), auth: auth(j) })),
        }
      : null;
    const proxy = f.useProxy
      ? { kind: f.proxyKind, host: f.proxyHost.trim(), port: port(f.proxyPort, 'proxy.port', raw, 1080), user: f.proxyUser.trim() || null }
      : null;
    const client_cert = f.useCert && f.tls !== 'disable' ? { cert: f.certFile.trim(), key: f.keyFile.trim() } : null;
    const base = { host: f.host.trim(), database: f.database.trim(), user: f.user.trim(), tls: f.tls, ca_file: f.caFile.trim() || null, ssh, proxy };
    if (d === 'clickhouse') return { target: { driver: d, ...base, port: port(f.port, 'port', raw, DEFAULT_PORT[d]), client_cert }, raw };
    if (d === 'redis') {
      const db = f.database.trim();
      const database = f.redisMode === 'cluster' ? 0 : hasVars(db) ? ((raw.database = db), 0) : Number.parseInt(db, 10) || 0;
      const mode: RedisMode = f.redisMode === 'sentinel' ? { kind: 'sentinel', master: f.sentinelMaster.trim() } : { kind: f.redisMode };
      const direct = f.redisMode === 'single';
      return {
        target: { driver: d, host: base.host, port: port(f.port, 'port', raw, DEFAULT_PORT[d]), database, user: base.user, tls: f.tls, ca_file: base.ca_file, ssh: direct ? ssh : null, proxy: direct ? proxy : null, client_cert, mode },
        raw,
      };
    }
    if (d === 'oracle') {
      const string = f.oraAddress === 'string';
      return {
        target: {
          driver: d,
          host: string ? '' : f.host.trim(),
          port: string ? DEFAULT_PORT[d] : port(f.port, 'port', raw, DEFAULT_PORT[d]),
          database: string ? '' : f.database.trim(),
          sid: !string && f.oraSid,
          connect_string: string ? f.connectString.trim() : '',
          user: f.user.trim(),
          tls: f.tls,
          ca_file: f.caFile.trim() || null,
          role: f.oraRole,
          ssh: string ? null : ssh,
          proxy: string ? null : proxy,
        },
        raw,
      };
    }
    if (d === 'cassandra') {
      return { target: { driver: d, ...base, port: port(f.port, 'port', raw, DEFAULT_PORT[d]), consistency: f.consistency, datacenter: f.datacenter.trim(), client_cert }, raw };
    }
    if (d === 'db2' || d === 'hana') {
      return { target: { driver: d, ...base, port: port(f.port, 'port', raw, DEFAULT_PORT[d]) }, raw };
    }
    if (d === 'firebird') {
      return { target: { driver: d, host: f.host.trim(), port: port(f.port, 'port', raw, DEFAULT_PORT[d]), database: f.database.trim(), user: f.user.trim(), role: null, ssh, proxy }, raw };
    }
    if (d === 'mongodb') {
      return { target: { driver: d, ...base, port: f.srv ? 0 : port(f.port, 'port', raw, DEFAULT_PORT[d]), srv: f.srv, auth_source: f.authSource.trim(), options: f.options.trim().replace(/^\?/, ''), ssh: f.srv ? null : ssh, proxy: f.srv ? null : proxy, client_cert }, raw };
    }
    return {
      target: {
        driver: d,
        host: f.host.trim(),
        port: port(f.port, 'port', raw, DEFAULT_PORT[d]),
        database: f.database.trim(),
        user: f.user.trim(),
        tls: f.tls,
        ca_file: f.caFile.trim() || null,
        ssh,
        proxy,
        client_cert,
        ...((d === 'postgres' || d === 'mysql') && f.flavor ? { flavor: f.flavor } : {}),
      },
      raw,
    };
  }
  if (d === 'odbc') return { target: { driver: d, dsn: f.odbcDsn.trim(), driver_path: f.odbcDsn.trim() ? '' : f.odbcDriver.trim(), connect_string: f.connectString.trim(), user: f.user.trim() }, raw };
  if (d === 'dynamodb') return { target: { driver: d, region: f.region.trim(), endpoint: f.endpoint.trim(), profile: f.awsProfile.trim(), user: f.user.trim() }, raw };
  if (d === 'libsql') {
    const proxy = f.useProxy ? { kind: f.proxyKind, host: f.proxyHost.trim(), port: port(f.proxyPort, 'proxy.port', raw, 1080), user: f.proxyUser.trim() || null } : null;
    return { target: f.location === 'remote' ? { driver: d, location: 'remote', url: f.url.trim(), proxy } : { driver: d, location: 'file', ...local }, raw };
  }
  return { target: { driver: d, ...local }, raw };
}

export function sessionOf(f: ProfileForm, network: boolean): ConnectionProfile['session'] {
  return {
    init: f.init.trim() ? f.init : '',
    keepalive_secs: network ? f.keepalive : 0,
    reconnect: f.reconnect,
    idle_minutes: f.idle,
    row_limit: wholeNumber(f.rowLimit, 1, 2_000_000) ?? null,
  };
}

export function fill(f: ProfileForm, from: ProfileForm, keys?: string[]) {
  const fields = keys ? keys.flatMap((k) => FIELD_KEYS[k] ?? []) : (Object.values(FIELD_KEYS).flat() as (keyof ProfileForm)[]);
  const target = f as unknown as Record<string, unknown>;
  for (const k of new Set(fields)) target[k] = structuredClone(from[k]);
  if (!keys) {
    f.driver = from.driver;
    f.flavor = from.flavor;
  }
}

export function varTexts(f: ProfileForm): string[] {
  const hops = [f.ssh, ...f.jumps].flatMap((h) => [h.host, h.port, h.user, h.key]);
  return [f.host, f.port, f.database, f.user, f.connectString, f.region, f.endpoint, f.awsProfile, f.datacenter, f.caFile, f.certFile, f.keyFile, ...(f.useSsh ? hops : []), f.proxyHost, f.proxyPort, f.proxyUser, f.path, f.url, f.init].filter(hasVars);
}

export function jumpSecrets(jumps: HopForm[], before: number): (string | null)[] {
  const out: (string | null)[] = jumps.map((j, i) => {
    if (j.method === 'agent') return j.saved === null ? null : '';
    if (j.secret) return j.secret;
    return j.saved === i ? null : '';
  });
  for (let i = jumps.length; i < before; i++) out.push('');
  return out;
}
