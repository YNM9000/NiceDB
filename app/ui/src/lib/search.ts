import type { Schema } from './api';
import type { Kinds, Match, QuickFilter, SearchSpec, SearchTable, TableHits } from './bindings';

export const ROW_LIMITS = [10, 50, 200] as const;
export const BUDGETS = [30, 60, 300, 900] as const;

export const HEAVY_ROWS = 1_000_000;
export const HEAVY_TABLES = 100;

const NUMBER = /^[+-]?\d+([.,]\d+)?$/;
const OTHER = /^(\d{4}-\d{2}(-\d{2})?|[0-9a-f]{8}-[0-9a-f]{4}-|\{|\[|\d{1,2}:\d{2})/i;

export const isNumber = (value: string) => NUMBER.test(value.trim());

export function kindsFor(value: string): Kinds {
  const v = value.trim();
  return { text: true, number: isNumber(v), other: OTHER.test(v) };
}

export function specOf(value: string, mode: Match, caseSensitive: boolean, kinds: Kinds): SearchSpec {
  return {
    value: value.trim(),
    mode,
    case_sensitive: caseSensitive,
    kinds: { ...kinds, number: kinds.number && isNumber(value) },
  };
}

const words = (filter: string) =>
  filter
    .toLowerCase()
    .split(/[\s,;]+/)
    .filter(Boolean);

export function searchTables(schema: Schema | null, namespaces: readonly string[], filter: string, views = false): SearchTable[] {
  if (!schema) return [];
  const w = words(filter);
  return schema.namespaces
    .filter((n) => namespaces.includes(n.name))
    .flatMap((n) =>
      [...n.tables, ...(views ? [...n.views, ...(n.materialized_views ?? [])] : [])]
        .filter((t) => !w.length || w.some((x) => t.name.toLowerCase().includes(x)))
        .map((t) => {
          const pk = t.columns.filter((c) => c.primary_key).map((c) => c.name);
          const unique = t.indexes.find((i) => i.unique && i.columns.length > 0)?.columns ?? [];
          return {
            table: { namespace: n.name, name: t.name },
            columns: t.columns.map((c) => ({
              name: c.name,
              type_name: c.type_name,
            })),
            key: pk.length ? pk : unique,
          };
        }),
    );
}

export interface Estimate {
  tables: number;
  columns: number;
  rows: number;
  unknown: number;
}

export function estimate(schema: Schema | null, tables: readonly SearchTable[], columns: readonly string[][]): Estimate {
  const out: Estimate = { tables: 0, columns: 0, rows: 0, unknown: 0 };
  tables.forEach((t, i) => {
    const cols = columns[i]?.length ?? 0;
    if (!cols) return;
    out.tables++;
    out.columns += cols;
    const rows = schema?.namespaces.find((n) => n.name === t.table.namespace)?.tables.find((r) => r.name === t.table.name)?.estimated_rows;
    if (rows === null || rows === undefined || rows < 0) out.unknown++;
    else out.rows += rows;
  });
  return out;
}

export const heavy = (e: Estimate) => e.rows > HEAVY_ROWS || e.tables > HEAVY_TABLES;

export function cellMatches(spec: SearchSpec, cell: string | null): boolean {
  if (cell === null || !spec.value) return false;
  const [a, b] = spec.case_sensitive ? [cell, spec.value] : [cell.toLowerCase(), spec.value.toLowerCase()];
  if (spec.mode === 'contains') return a.includes(b) || (isNumber(spec.value) && a.includes(b.replace(',', '.')));
  if (a === b) return true;
  return isNumber(spec.value) && isNumber(cell) && Number(cell.replace(',', '.')) === Number(spec.value.replace(',', '.'));
}

export function rowFilters(hits: TableHits, row: readonly (string | null)[], spec: SearchSpec): QuickFilter[] {
  const at = (c: string) => row[hits.shown.indexOf(c)] ?? null;
  if (hits.key.length && hits.key.every((k) => at(k) !== null)) return hits.key.map((k) => ({ column: k, op: 'eq', value: at(k) }));
  const found = hits.columns.find((c) => cellMatches(spec, at(c.column)))?.column ?? hits.columns[0]?.column;
  if (!found) return [];
  const value = at(found);
  return [{ column: found, op: value === null ? 'is_null' : 'eq', value }];
}

let seed: { namespace: string } | null = null;

export const seedSearch = (namespace: string) => (seed = { namespace });

export function takeSearchSeed(): string | null {
  const s = seed?.namespace ?? null;
  seed = null;
  return s;
}
