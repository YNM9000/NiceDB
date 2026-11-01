import type { Dialect, Flavor as Edition, DiffItem_Serialize as DiffItem, FileFormat, Grant_Serialize as Grant, Principal_Serialize as Principal, Progress_Serialize as Progress } from './bindings';
import { m } from '../paraglide/messages';
import { getLocale } from '../paraglide/runtime';
import { isRelational, qualifiedName, quoteName } from './sqlname';

export type ToolKind = 'activity' | 'monitor' | 'space' | 'users' | 'compare' | 'rows' | 'backup' | 'transfer' | 'seclab' | 'search' | 'generate' | 'server' | 'cluster';

export type Flavor = 'postgres' | 'cockroach' | 'mysql' | 'mssql' | 'sqlite' | 'duckdb' | 'clickhouse' | 'firebird' | 'hana' | 'oracle' | 'db2' | 'ansi';

export function flavorOf(d: Dialect, product: string): Flavor {
  if (d === 'postgres') return product === 'CockroachDB' ? 'cockroach' : 'postgres';
  if (d === 'ansi_backtick') return 'ansi';
  return isRelational(d) ? d : 'postgres';
}

export type Variant = Flavor | 'yugabyte' | 'timescale' | 'mariadb' | 'tidb';

export function variantOf(d: Dialect, product: string, edition: Edition | null = null): Variant {
  const p = product.toLowerCase();
  if (d === 'postgres' && (edition === 'yugabyte' || p.includes('yugabyte'))) return 'yugabyte';
  if (d === 'postgres' && (edition === 'timescale' || p.includes('timescale'))) return 'timescale';
  if (d === 'mysql' && (edition === 'tidb' || p.includes('tidb'))) return 'tidb';
  if (d === 'mysql' && p.includes('mariadb')) return 'mariadb';
  return flavorOf(d, product);
}

export const variantOfSession = (info: { dialect?: Dialect | null; server: { product: string }; edition?: { flavor: Edition } | null }): Variant => variantOf(info.dialect ?? 'postgres', info.server.product, info.edition?.flavor ?? null);

export const baseOf = (v: Variant): Flavor => (v === 'yugabyte' || v === 'timescale' ? 'postgres' : v === 'mariadb' || v === 'tidb' ? 'mysql' : v);

export function dropIndexSql(d: Dialect, flavor: Flavor, ns: string, name: string, table: string, db: string | null): string {
  if (d === 'mysql' || d === 'mssql') return `DROP INDEX ${qualifiedName(d, '', name)} ON ${qualifiedName(d, ns, table, db)};`;
  if (d === 'clickhouse') return `ALTER TABLE ${qualifiedName(d, ns, table, db)} DROP INDEX ${qualifiedName(d, '', name)};`;
  if (flavor === 'cockroach') return `DROP INDEX ${qualifiedName(d, ns, table, db)}@${qualifiedName(d, '', name)};`;
  return `DROP INDEX ${qualifiedName(d, ns, name, db)}${d === 'oracle' ? '' : ';'}`;
}

export const DIALECT_NAME: Record<Dialect, string> = { postgres: 'PostgreSQL', mysql: 'MySQL / MariaDB', sqlite: 'SQLite', mssql: 'SQL Server', duckdb: 'DuckDB', clickhouse: 'ClickHouse', firebird: 'Firebird', hana: 'SAP HANA', db2: 'Db2', ansi: 'ODBC', ansi_backtick: 'ODBC', cql: 'CQL', partiql: 'PartiQL', opensearch: 'OpenSearch SQL', elasticsearch: 'Elasticsearch SQL', oracle: 'Oracle' };

const tag = () => (getLocale() === 'it' ? 'it-IT' : 'en-GB');

const UNITS = ['B', 'kB', 'MB', 'GB', 'TB'];

export function bytes(n: number | null | undefined): string {
  if (n === null || n === undefined || !Number.isFinite(n)) return '—';
  let v = Math.max(0, n);
  let u = 0;
  while (v >= 1024 && u < UNITS.length - 1) {
    v /= 1024;
    u++;
  }
  const digits = u === 0 || v >= 100 ? 0 : 1;
  return `${new Intl.NumberFormat(tag(), { maximumFractionDigits: digits }).format(v)} ${UNITS[u]}`;
}

export function duration(ms: number | null | undefined): string {
  if (ms === null || ms === undefined || !Number.isFinite(ms)) return '—';
  const s = Math.floor(ms / 1000);
  if (ms < 1000) return `${Math.max(0, Math.round(ms))} ms`;
  if (s < 60) return `${s} s`;
  const m = Math.floor(s / 60);
  if (m < 60) return s % 60 ? `${m} min ${s % 60} s` : `${m} min`;
  const h = Math.floor(m / 60);
  return m % 60 ? `${h} h ${m % 60} min` : `${h} h`;
}

export function fraction(p: Progress | null): number | null {
  if (!p || p.total === null || p.total <= 0) return null;
  return Math.min(1, Math.max(0, (p.done ?? 0) / p.total));
}

export function formatOf(path: string): FileFormat | null {
  const ext = path.split(/[\\/]/).pop()?.split('.').pop()?.toLowerCase() ?? '';
  if (ext === 'csv') return 'csv';
  if (ext === 'tsv' || ext === 'tab') return 'tsv';
  if (ext === 'json' || ext === 'ndjson' || ext === 'jsonl') return 'json';
  if (ext === 'xlsx' || ext === 'xlsm' || ext === 'xls' || ext === 'ods') return 'xlsx';
  if (ext === 'parquet' || ext === 'pq') return 'parquet';
  return null;
}

export const fileName = (path: string) => path.split(/[\\/]/).pop() ?? path;

export function plainName(s: string): string {
  return s
    .normalize('NFD')
    .replace(/[̀-ͯ]/g, '')
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, '_')
    .replace(/^_+|_+$/g, '');
}

export function tableFromFile(path: string): string {
  const base = fileName(path).replace(/\.[^.]+$/, '');
  const name = plainName(base) || m.file_imported_table();
  return /^[0-9]/.test(name) ? `t_${name}` : name;
}

export function matchColumns(source: readonly string[], target: readonly string[]): (number | null)[] {
  const keys = source.map(plainName);
  return target.map((t) => {
    const exact = source.indexOf(t);
    if (exact >= 0) return exact;
    const i = keys.indexOf(plainName(t));
    return i >= 0 ? i : null;
  });
}

export function scriptText(statements: readonly string[]): string {
  return statements.map((s) => `${s.trimEnd()};`).join('\n\n');
}

export function diffGroups(items: readonly DiffItem[]): { head: DiffItem; children: DiffItem[] }[] {
  const out: { head: DiffItem; children: DiffItem[] }[] = [];
  for (const it of items) {
    const parent = it.table ? out.find((g) => g.head.kind === 'table' && g.head.name === it.table) : undefined;
    if (parent) parent.children.push(it);
    else out.push({ head: it, children: [] });
  }
  return out;
}

const literal = (s: string) => `'${s.replaceAll('\\', '\\\\').replaceAll("'", "''")}'`;

export function account(d: Dialect, name: string, host: string | null = null): string {
  return d === 'mysql' ? `${literal(name)}@${literal(host || '%')}` : quoteName(d, name);
}

export interface RoleDraft {
  name: string;
  host: string;
  login: boolean;
  createDb: boolean;
  createRole: boolean;
  memberOf: string[];
}

const flag = (on: boolean, word: string) => (on ? word : `NO${word}`);

const roleRef = (d: Dialect, v: Variant, g: string) => (v === 'mariadb' ? quoteName(d, g) : account(d, g));

export function createRoleSql(d: Dialect, r: RoleDraft, v: Variant = flavorOf(d, '')): string {
  const who = account(d, r.name, r.host);
  if (d === 'clickhouse') {
    const lines = [r.login ? `CREATE USER ${who} HOST NONE;` : `CREATE ROLE ${who};`];
    if (r.memberOf.length) lines.push(`GRANT ${r.memberOf.map((g) => quoteName(d, g)).join(', ')} TO ${who};`);
    return lines.join('\n');
  }
  if (d === 'mysql') {
    const maria = v === 'mariadb';
    const self = r.login || !maria ? who : quoteName(d, r.name);
    const lines = [r.login ? `CREATE USER ${self} ACCOUNT LOCK;` : `CREATE ROLE ${self};`];
    if (r.memberOf.length) lines.push(`GRANT ${r.memberOf.map((g) => roleRef(d, v, g)).join(', ')} TO ${self};`);
    if (r.memberOf.length && r.login) lines.push(maria ? `SET DEFAULT ROLE ${roleRef(d, v, r.memberOf[0] ?? '')} FOR ${self};` : `SET DEFAULT ROLE ALL TO ${self};`);
    return lines.join('\n');
  }
  if (d === 'mssql') return [`CREATE USER ${who} WITHOUT LOGIN;`, ...r.memberOf.map((g) => `ALTER ROLE ${quoteName(d, g)} ADD MEMBER ${who};`)].join('\n');
  if (d === 'oracle') {
    const lines = [r.login ? `CREATE USER ${who} NO AUTHENTICATION;` : `CREATE ROLE ${who};`];
    const privileges = [r.login && 'CREATE SESSION', r.createDb && 'CREATE USER', r.createRole && 'CREATE ROLE'].filter((x): x is string => !!x);
    if (privileges.length) lines.push(`GRANT ${privileges.join(', ')} TO ${who};`);
    for (const g of r.memberOf) lines.push(`GRANT ${quoteName(d, g)} TO ${who};`);
    return lines.join('\n');
  }
  const lines = [`CREATE ROLE ${who} ${[flag(r.login, 'LOGIN'), flag(r.createDb, 'CREATEDB'), flag(r.createRole, 'CREATEROLE')].join(' ')};`];
  for (const g of r.memberOf) lines.push(`GRANT ${quoteName(d, g)} TO ${who};`);
  return lines.join('\n');
}

export function alterRoleSql(d: Dialect, before: Principal, r: RoleDraft, v: Variant = flavorOf(d, '')): string {
  const who = account(d, before.name, before.host);
  const has = (a: Principal['attributes'][number]) => before.attributes.includes(a);
  const lines: string[] = [];
  if (d === 'mysql') {
    const self = v === 'mariadb' && !has('login') ? quoteName(d, before.name) : who;
    if (has('login') && r.login === has('locked')) lines.push(`ALTER USER ${who} ACCOUNT ${r.login ? 'UNLOCK' : 'LOCK'};`);
    for (const g of r.memberOf.filter((g) => !before.member_of.includes(g))) lines.push(`GRANT ${roleRef(d, v, g)} TO ${self};`);
    for (const g of before.member_of.filter((g) => !r.memberOf.includes(g))) lines.push(`REVOKE ${roleRef(d, v, g)} FROM ${self};`);
    return lines.join('\n');
  }
  if (d === 'clickhouse') {
    const kind = has('login') ? 'USER' : 'ROLE';
    for (const g of r.memberOf.filter((g) => !before.member_of.includes(g))) lines.push(`GRANT ${quoteName(d, g)} TO ${who};`);
    for (const g of before.member_of.filter((g) => !r.memberOf.includes(g))) lines.push(`REVOKE ${quoteName(d, g)} FROM ${who};`);
    if (r.name && r.name !== before.name) lines.push(`ALTER ${kind} ${who} RENAME TO ${quoteName(d, r.name)};`);
    return lines.join('\n');
  }
  if (d === 'mssql') {
    for (const g of r.memberOf.filter((g) => !before.member_of.includes(g))) lines.push(`ALTER ROLE ${quoteName(d, g)} ADD MEMBER ${who};`);
    for (const g of before.member_of.filter((g) => !r.memberOf.includes(g))) lines.push(`ALTER ROLE ${quoteName(d, g)} DROP MEMBER ${who};`);
    if (r.name && r.name !== before.name) lines.push(`${before.attributes.includes('login') ? 'ALTER USER' : 'ALTER ROLE'} ${who} WITH NAME = ${quoteName(d, r.name)};`);
    return lines.join('\n');
  }
  if (d === 'oracle') {
    const toggled: [boolean, boolean, string][] = [
      [has('login'), r.login, 'CREATE SESSION'],
      [has('create_db'), r.createDb, 'CREATE USER'],
      [has('create_role'), r.createRole, 'CREATE ROLE'],
    ];
    const granted = toggled.filter(([was, now]) => !was && now).map(([, , p]) => p);
    const revoked = toggled.filter(([was, now]) => was && !now).map(([, , p]) => p);
    if (granted.length) lines.push(`GRANT ${granted.join(', ')} TO ${who};`);
    if (revoked.length) lines.push(`REVOKE ${revoked.join(', ')} FROM ${who};`);
    for (const g of r.memberOf.filter((g) => !before.member_of.includes(g))) lines.push(`GRANT ${quoteName(d, g)} TO ${who};`);
    for (const g of before.member_of.filter((g) => !r.memberOf.includes(g))) lines.push(`REVOKE ${quoteName(d, g)} FROM ${who};`);
    return lines.join('\n');
  }
  const changes = [
    has('login') !== r.login && flag(r.login, 'LOGIN'),
    has('create_db') !== r.createDb && flag(r.createDb, 'CREATEDB'),
    has('create_role') !== r.createRole && flag(r.createRole, 'CREATEROLE'),
  ].filter((x): x is string => !!x);
  if (changes.length) lines.push(`ALTER ROLE ${who} ${changes.join(' ')};`);
  for (const g of r.memberOf.filter((g) => !before.member_of.includes(g))) lines.push(`GRANT ${quoteName(d, g)} TO ${who};`);
  for (const g of before.member_of.filter((g) => !r.memberOf.includes(g))) lines.push(`REVOKE ${quoteName(d, g)} FROM ${who};`);
  if (r.name && r.name !== before.name) lines.push(`ALTER ROLE ${who} RENAME TO ${quoteName(d, r.name)};`);
  return lines.join('\n');
}

export function dropRoleSql(d: Dialect, p: Principal, v: Variant = flavorOf(d, '')): string {
  if (d === 'mysql' && !p.attributes.includes('login')) return `DROP ROLE ${v === 'mariadb' ? quoteName(d, p.name) : account(d, p.name, p.host)};`;
  if (d === 'clickhouse') return `${p.attributes.includes('login') ? 'DROP USER' : 'DROP ROLE'} ${account(d, p.name)};`;
  if (d === 'mssql' || d === 'oracle') return `${p.attributes.includes('login') ? 'DROP USER' : 'DROP ROLE'} ${account(d, p.name)};`;
  return d === 'mysql' ? `DROP USER ${account(d, p.name, p.host)};` : `DROP ROLE ${account(d, p.name)};`;
}

export type GrantKind = 'table' | 'schema' | 'database';

export type UserAction = { mode: 'create' } | { mode: 'alter' | 'drop' | 'grant'; principal: Principal } | { mode: 'revoke'; principal: Principal; grants: Grant[] };

export const PRIVILEGES: Record<'postgres' | 'cockroach' | 'mysql' | 'mssql' | 'clickhouse' | 'mongodb' | 'oracle', Record<GrantKind, string[]>> = {
  oracle: {
    table: ['SELECT', 'INSERT', 'UPDATE', 'DELETE', 'READ', 'ALTER', 'INDEX', 'REFERENCES'],
    schema: ['SELECT ANY TABLE', 'INSERT ANY TABLE', 'UPDATE ANY TABLE', 'DELETE ANY TABLE', 'EXECUTE ANY PROCEDURE'],
    database: ['CREATE SESSION', 'CREATE TABLE', 'CREATE VIEW', 'CREATE SEQUENCE', 'CREATE PROCEDURE', 'CREATE TRIGGER', 'UNLIMITED TABLESPACE'],
  },
  clickhouse: {
    table: ['SELECT', 'INSERT', 'ALTER', 'TRUNCATE', 'OPTIMIZE', 'DROP TABLE', 'SHOW'],
    schema: [],
    database: ['SELECT', 'INSERT', 'ALTER', 'CREATE TABLE', 'DROP TABLE', 'SHOW'],
  },
  mongodb: {
    table: ['find', 'insert', 'update', 'remove', 'createIndex', 'dropIndex'],
    schema: [],
    database: ['read', 'readWrite', 'dbAdmin', 'dbOwner', 'userAdmin'],
  },
  postgres: {
    table: ['SELECT', 'INSERT', 'UPDATE', 'DELETE', 'TRUNCATE', 'REFERENCES', 'TRIGGER'],
    schema: ['USAGE', 'CREATE'],
    database: ['CONNECT', 'CREATE', 'TEMPORARY'],
  },
  cockroach: {
    table: ['SELECT', 'INSERT', 'UPDATE', 'DELETE', 'DROP', 'ZONECONFIG'],
    schema: ['USAGE', 'CREATE'],
    database: ['CONNECT', 'CREATE', 'DROP', 'ZONECONFIG'],
  },
  mssql: {
    table: ['SELECT', 'INSERT', 'UPDATE', 'DELETE', 'REFERENCES', 'ALTER', 'CONTROL'],
    schema: ['SELECT', 'INSERT', 'UPDATE', 'DELETE', 'EXECUTE', 'ALTER', 'CONTROL'],
    database: ['CONNECT', 'CREATE TABLE', 'CREATE VIEW', 'CREATE PROCEDURE', 'CREATE FUNCTION', 'CREATE SCHEMA'],
  },
  mysql: {
    table: ['SELECT', 'INSERT', 'UPDATE', 'DELETE', 'ALTER', 'INDEX', 'DROP'],
    schema: ['SELECT', 'INSERT', 'UPDATE', 'DELETE', 'CREATE', 'ALTER', 'DROP', 'EXECUTE'],
    database: [],
  },
};

export interface GrantDraft {
  privileges: string[];
  kind: GrantKind;
  schema: string;
  object: string;
}

function target(d: Dialect, kind: string, schema: string | null, object: string | null): string {
  const q = (n: string) => quoteName(d, n);
  const name = `${schema ? `${q(schema)}.` : ''}${q(object ?? '')}`;
  if (d === 'mssql') {
    if (kind === 'database') return '';
    return kind === 'schema' ? `ON SCHEMA::${q(schema ?? '')}` : `ON ${name}`;
  }
  if (d === 'oracle') {
    if (kind === 'database' || kind === 'global') return '';
    return kind === 'schema' ? `ON SCHEMA ${q(schema ?? '')}` : `ON ${name}`;
  }
  if (d === 'mysql') return `ON ${kind === 'schema' ? `${q(schema ?? '')}.*` : `${q(schema ?? '')}.${q(object ?? '')}`}`;
  if (d === 'clickhouse') return kind === 'database' ? `ON ${q(object ?? '')}.*` : `ON ${q(schema ?? '')}.${q(object ?? '')}`;
  if (kind === 'database') return `ON DATABASE ${q(object ?? '')}`;
  if (kind === 'schema') return `ON SCHEMA ${q(schema ?? '')}`;
  return `ON ${kind === 'sequence' ? 'SEQUENCE' : 'TABLE'} ${name}`;
}

const clause = (head: string, on: string, tail: string) => [head, on, tail].filter(Boolean).join(' ');

export function grantSql(d: Dialect, g: GrantDraft, to: { name: string; host: string | null }): string {
  if (!g.privileges.length) return '';
  return `${clause(`GRANT ${g.privileges.join(', ')}`, target(d, g.kind, g.schema, g.object), `TO ${account(d, to.name, to.host)}`)};`;
}

export function revokeSql(d: Dialect, g: Grant): string | null {
  if (g.privilege === 'OWNER') return null;
  const at = g.grantee.lastIndexOf('@');
  const who =
    g.grantee === 'PUBLIC' ? 'PUBLIC' : d === 'mysql' && at > 0 ? account(d, g.grantee.slice(0, at), g.grantee.slice(at + 1)) : account(d, g.grantee);
  const on = g.kind === 'global' && d !== 'oracle' ? 'ON *.*' : target(d, g.kind, g.schema, g.object);
  if (d === 'clickhouse') {
    const col = /^(.+)\((.+)\)$/.exec(g.privilege);
    const what = col ? `${col[1]}(${quoteName(d, col[2] ?? '')})` : g.privilege;
    return `${clause(`REVOKE ${what}`, on, `FROM ${who}`)};`;
  }
  return `${clause(`REVOKE ${g.privilege}`, on, `FROM ${who}`)};`;
}
