import type { Dialect } from './bindings';
import { needsQuote } from './complete/dialects';

export type NativeDialect = 'cql' | 'partiql' | 'opensearch' | 'elasticsearch';
export type RelationalDialect = Exclude<Dialect, NativeDialect>;

const NATIVE: readonly Dialect[] = ['cql', 'partiql', 'opensearch', 'elasticsearch'];
export const isRelational = (d: Dialect): d is RelationalDialect => !NATIVE.includes(d);

export function quoteName(d: Dialect, name: string): string {
  if (!needsQuote(name, d)) return name;
  if (d === 'mssql') return `[${name.replaceAll(']', ']]')}]`;
  const q = d === 'mysql' || d === 'clickhouse' || d === 'opensearch' || d === 'ansi_backtick' ? '`' : '"';
  const body = d === 'clickhouse' ? name.replaceAll('\\', '\\\\') : name;
  return q + body.replaceAll(q, q + q) + q;
}

export const isImplicit = (d: Dialect, ns: string, current: string | null) => (d === 'firebird' ? true : d === 'oracle' ? false : d === 'postgres' ? ns === 'public' : d === 'sqlite' || d === 'duckdb' ? ns === 'main' : d === 'mssql' ? ns === 'dbo' : ns === current);

export function qualifiedName(d: Dialect, ns: string, name: string, current: string | null = null): string {
  return isImplicit(d, ns, current) || !ns ? quoteName(d, name) : `${quoteName(d, ns)}.${quoteName(d, name)}`;
}

export function limitedSql(d: Dialect, list: string, rest: string, limit = 1000): string {
  if (d === 'mssql') return `SELECT TOP (${limit}) ${list} ${rest};`;
  if (d === 'oracle') return `SELECT ${list} ${rest} FETCH FIRST ${limit} ROWS ONLY`;
  if (d === 'partiql') return `SELECT ${list} ${rest};`;
  return d === 'firebird' || d === 'db2' || d === 'ansi' || d === 'ansi_backtick' ? `SELECT ${list} ${rest} FETCH FIRST ${limit} ROWS ONLY;` : `SELECT ${list} ${rest} LIMIT ${limit};`;
}

export function selectSql(d: Dialect, ns: string, name: string, current: string | null = null, limit = 1000): string {
  return limitedSql(d, '*', `FROM ${qualifiedName(d, ns, name, current)}`, limit);
}
