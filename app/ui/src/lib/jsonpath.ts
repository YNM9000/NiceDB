import type { Dialect } from './bindings';
import { quoteName } from './sqlname';

export type Seg = string | number;
export type Path = readonly Seg[];

const PLAIN_KEY = /^[A-Za-z_$][\w$]*$/;

export const pathKey = (p: Path) => JSON.stringify(p);

export function jsonPath(p: Path): string {
  let out = '$';
  for (const s of p) out += typeof s === 'number' ? `[${s}]` : PLAIN_KEY.test(s) ? `.${s}` : `[${JSON.stringify(s)}]`;
  return out;
}

function dottedPath(p: Path): string {
  let out = '$';
  for (const s of p) out += typeof s === 'number' ? `[${s}]` : PLAIN_KEY.test(s) ? `.${s}` : `.${JSON.stringify(s)}`;
  return out;
}

const sqlString = (s: string) => `'${s.replaceAll("'", "''")}'`;

function pgElement(s: Seg): string {
  const t = String(s);
  return /^[^\s{},"\\]+$/.test(t) ? t : `"${t.replaceAll('\\', '\\\\').replaceAll('"', '\\"')}"`;
}

export function sqlPath(dialect: Dialect, column: string, p: Path): string {
  const col = quoteName(dialect, column);
  switch (dialect) {
    case 'postgres':
      return p.length ? `${col} #>> ${sqlString(`{${p.map(pgElement).join(',')}}`)}` : `${col}::text`;
    case 'mysql':
      return `JSON_UNQUOTE(JSON_EXTRACT(${col}, ${sqlString(dottedPath(p))}))`;
    case 'mssql':
      return `JSON_VALUE(${col}, ${sqlString(dottedPath(p))})`;
    case 'sqlite':
    case 'duckdb':
      return `json_extract(${col}, ${sqlString(dottedPath(p))})`;
    case 'clickhouse':
    case 'hana':
    case 'oracle':
    case 'db2':
    case 'ansi':
    case 'ansi_backtick':
      return `JSON_VALUE(${col}, ${sqlString(dottedPath(p))})`;
    case 'partiql':
      return col + p.map((s) => (typeof s === 'number' ? `[${s}]` : `.${quoteName(dialect, s)}`)).join('');
    case 'opensearch':
    case 'elasticsearch':
      return [col, ...p.filter((s): s is string => typeof s === 'string').map((s) => quoteName(dialect, s))].join('.');
    case 'firebird':
    case 'cql':
      return col;
  }
}

export type JsonType = 'object' | 'array' | 'string' | 'number' | 'boolean' | 'null';

export const typeOf = (v: unknown): JsonType => (v === null ? 'null' : Array.isArray(v) ? 'array' : typeof v === 'object' ? 'object' : typeof v === 'string' ? 'string' : typeof v === 'number' ? 'number' : 'boolean');

export function children(v: unknown): [Seg, unknown][] {
  if (Array.isArray(v)) return v.map((x, i) => [i, x]);
  if (v !== null && typeof v === 'object') return Object.entries(v);
  return [];
}

export function at(root: unknown, p: Path): unknown {
  let v = root;
  for (const s of p) v = children(v).find(([k]) => k === s)?.[1];
  return v;
}

export const scalarText = (v: unknown) => (typeof v === 'string' ? JSON.stringify(v) : String(v));

export interface Found {
  hits: Set<string>;
  open: Set<string>;
  count: number;
}

export function search(root: unknown, query: string, limit = 2000): Found {
  const q = query.trim().toLowerCase();
  const found: Found = { hits: new Set(), open: new Set(), count: 0 };
  if (!q) return found;
  const walk = (v: unknown, p: Seg[]) => {
    for (const [k, x] of children(v)) {
      if (found.count >= limit) return;
      const here = [...p, k];
      const scalar = typeOf(x) !== 'object' && typeOf(x) !== 'array';
      if ((typeof k === 'string' && k.toLowerCase().includes(q)) || (scalar && String(x).toLowerCase().includes(q))) {
        found.hits.add(pathKey(here));
        found.count++;
        for (let i = 0; i < here.length; i++) found.open.add(pathKey(here.slice(0, i)));
      }
      walk(x, here);
    }
  };
  walk(root, []);
  return found;
}

export function initialOpen(root: unknown): Set<string> {
  const open = new Set([pathKey([])]);
  const kids = children(root);
  if (kids.length <= 12) for (const [k, x] of kids) if (children(x).length > 0 && children(x).length <= 20) open.add(pathKey([k]));
  return open;
}
