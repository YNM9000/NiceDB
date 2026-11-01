import type { Column, Dialect } from './bindings';
import { quoteName } from './sqlname';
import { m } from '../paraglide/messages';

export type Cell = string | null;
export type CopyFormat = 'tsv' | 'csv' | 'json' | 'insert';

const NUMERIC = /^(int[248]?|integer|smallint|bigint|numeric|decimal|real|double precision|float[48]?|double|money|oid|serial|bigserial|number|binary_float|binary_double)\b/i;

export const isNumericType = (type: string | null) => !!type && NUMERIC.test(type);

function delimited(sep: string, columns: readonly Column[], rows: readonly Cell[][], header: boolean): string {
  const quote = (v: Cell) => {
    if (v === null) return '';
    return /["\r\n]/.test(v) || v.includes(sep) ? `"${v.replaceAll('"', '""')}"` : v;
  };
  const lines = rows.map((r) => r.map(quote).join(sep));
  if (header) lines.unshift(columns.map((c) => quote(c.name)).join(sep));
  return lines.join('\n');
}

function jsonValue(v: Cell, type: string | null): unknown {
  if (v === null) return null;
  if (isNumericType(type)) {
    const n = Number(v);
    if (Number.isFinite(n) && (Number.isSafeInteger(n) || !/^-?\d+$/.test(v))) return n;
  }
  if (type === 'bool' || type === 'boolean') return v === 't' || v === 'true' || v === '1';
  return v;
}


function sqlValue(d: Dialect, v: Cell, type: string | null) {
  if (v === null) return 'NULL';
  if (isNumericType(type) && /^-?\d+(\.\d+)?(e[+-]?\d+)?$/i.test(v)) return v;
  if (type && (/^(bool|boolean)$/i.test(type) || (d === 'mssql' && /^bit$/i.test(type))) && /^(t|f|true|false|1|0)$/i.test(v)) {
    const on = /^(t|true|1)$/i.test(v);
    return d === 'mssql' || d === 'sqlite' ? (on ? '1' : '0') : on ? 'TRUE' : 'FALSE';
  }
  if (d === 'oracle' && type) {
    const lit = `'${v.replaceAll("'", "''")}'`;
    if (/^date\b/i.test(type)) return `TO_DATE(${lit}, 'YYYY-MM-DD HH24:MI:SS')`;
    if (/^timestamp.*time zone/i.test(type)) return `TO_TIMESTAMP_TZ(${lit}, 'YYYY-MM-DD HH24:MI:SS.FF TZH:TZM')`;
    if (/^timestamp/i.test(type)) return `TO_TIMESTAMP(${lit}, 'YYYY-MM-DD HH24:MI:SS.FF')`;
    if (/^(raw|blob|long raw)\b/i.test(type) && /^\\x[0-9a-f]*$/i.test(v)) return `HEXTORAW('${v.slice(2).toUpperCase()}')`;
  }
  const body = d === 'mysql' || d === 'clickhouse' ? v.replaceAll('\\', '\\\\') : v;
  return `'${body.replaceAll("'", "''")}'`;
}

function docValue(v: Cell, type: string | null): unknown {
  if (v === null) return null;
  if ((type === 'int' || type === 'double') && Number.isFinite(Number(v))) return Number.isSafeInteger(Number(v)) || !/^-?\d+$/.test(v) ? Number(v) : v;
  if (type === 'bool') return v === 'true';
  if (type === 'array' || type === 'document') {
    try {
      return JSON.parse(v) as unknown;
    } catch {
      return v;
    }
  }
  return v;
}

const partiqlString = (s: string) => `'${s.replaceAll("'", "''")}'`;

function partiqlValue(v: unknown): string {
  if (v === null || v === undefined) return 'NULL';
  if (typeof v === 'number' || typeof v === 'boolean') return String(v);
  if (typeof v === 'string') return partiqlString(v);
  if (Array.isArray(v)) return `[${v.map(partiqlValue).join(', ')}]`;
  const o = v as Record<string, unknown>;
  const keys = Object.keys(o);
  if (keys.length === 1 && Array.isArray(o.$set)) return `<<${o.$set.map(partiqlValue).join(', ')}>>`;
  if (keys.length === 1 && typeof o.$numberDecimal === 'string' && /^-?\d+(\.\d+)?(e[+-]?\d+)?$/i.test(o.$numberDecimal)) return o.$numberDecimal;
  if (keys.length === 1 && typeof o.$binary === 'string') return partiqlString(o.$binary);
  return `{${keys.map((k) => `${partiqlString(k)}: ${partiqlValue(o[k])}`).join(', ')}}`;
}

const cqlJsonKey = (k: string) => (k.toLowerCase() === k ? k : `"${k}"`);

export const insertable = (d: Dialect | null | undefined) => d !== 'opensearch' && d !== 'elasticsearch';

const PART = String.raw`(?:"(?:[^"]|"")+"|\`(?:[^\`]|\`\`)+\`|\[(?:[^\]]|\]\])+\]|[\w$]+)`;
const FROM = new RegExp(String.raw`\bfrom\s+(${PART}(?:\s*\.\s*${PART}){0,2})`, 'i');

export function tablePartsFromSql(sql: string): string[] | null {
  const found = FROM.exec(sql)?.[1];
  if (!found) return null;
  return [...found.matchAll(new RegExp(PART, 'g'))].map(([p]) => {
    if (p.startsWith('"')) return p.slice(1, -1).replaceAll('""', '"');
    if (p.startsWith('`')) return p.slice(1, -1).replaceAll('``', '`');
    if (p.startsWith('[')) return p.slice(1, -1).replaceAll(']]', ']');
    return p;
  });
}

export function tableFromSql(sql: string): string | null {
  return FROM.exec(sql)?.[1] ?? null;
}

export function render(format: CopyFormat, columns: readonly Column[], rows: readonly Cell[][], opts: { header?: boolean; table?: string | null; dialect?: Dialect | null } = {}): string {
  switch (format) {
    case 'tsv':
      return delimited('\t', columns, rows, opts.header ?? false);
    case 'csv':
      return delimited(',', columns, rows, opts.header ?? true);
    case 'json':
      return JSON.stringify(
        rows.map((r) => Object.fromEntries(columns.map((c, i) => [c.name, jsonValue(r[i] ?? null, c.type_name ?? null)]))),
        null,
        2,
      );
    case 'insert': {
      const d = opts.dialect ?? 'postgres';
      const table = opts.table ?? m.copy_table_placeholder();
      const doc = (r: readonly Cell[], key = (k: string) => k) => Object.fromEntries(columns.map((c, i) => [key(c.name), docValue(r[i] ?? null, c.type_name ?? null)]));
      if (d === 'cql') return rows.map((r) => `INSERT INTO ${table} JSON ${partiqlString(JSON.stringify(doc(r, cqlJsonKey)))};`).join('\n');
      if (d === 'partiql') return rows.map((r) => `INSERT INTO ${table} VALUE ${partiqlValue(doc(r))};`).join('\n');
      const cols = columns.map((c) => quoteName(d, c.name)).join(', ');
      return rows.map((r) => `INSERT INTO ${table} (${cols}) VALUES (${columns.map((c, i) => sqlValue(d, r[i] ?? null, c.type_name ?? null)).join(', ')});`).join('\n');
    }
  }
}

export function recordText(columns: readonly Column[], row: readonly Cell[]): string {
  return columns.map((c, i) => `${c.name}\t${row[i] ?? 'NULL'}`).join('\n');
}
