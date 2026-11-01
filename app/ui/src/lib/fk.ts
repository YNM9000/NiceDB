import type { ColumnInfo, ForeignKey, QuickFilter, Relation_Serialize as Relation, Schema_Serialize as Schema } from './bindings';

export interface TableName {
  namespace: string;
  name: string;
}

export interface Incoming {
  namespace: string;
  table: string;
  fk: ForeignKey;
}

type Cell = string | null;

const sameTable = (a: TableName, b: TableName) => a.namespace === b.namespace && a.name === b.name;

export const targetOf = (owner: string, fk: ForeignKey): TableName => ({ namespace: fk.ref_namespace || owner, name: fk.ref_table });

function relation(schema: Schema | null, t: TableName): Relation | null {
  return schema?.namespaces.find((n) => n.name === t.namespace)?.tables.find((r) => r.name === t.name) ?? null;
}

export function outgoing(schema: Schema | null, t: TableName): ForeignKey[] {
  return relation(schema, t)?.foreign_keys.filter((fk) => fk.columns.length > 0 && fk.columns.length === fk.ref_columns.length) ?? [];
}

export function incoming(schema: Schema | null, t: TableName): Incoming[] {
  const out: Incoming[] = [];
  for (const ns of schema?.namespaces ?? [])
    for (const rel of ns.tables)
      for (const fk of rel.foreign_keys)
        if (fk.columns.length > 0 && fk.columns.length === fk.ref_columns.length && sameTable(targetOf(ns.name, fk), t)) out.push({ namespace: ns.name, table: rel.name, fk });
  return out.sort((a, b) => a.table.localeCompare(b.table) || a.fk.name.localeCompare(b.fk.name));
}

export function fkOfColumn(fks: readonly ForeignKey[], column: string): ForeignKey | null {
  const all = fks.filter((fk) => fk.columns.includes(column));
  return all.find((fk) => fk.columns.length === 1) ?? all[0] ?? null;
}

function valuesOf(names: readonly string[], columns: readonly string[], row: readonly Cell[]): string[] | null {
  const out: string[] = [];
  for (const n of names) {
    const v = row[columns.indexOf(n)];
    if (v === null || v === undefined) return null;
    out.push(v);
  }
  return out;
}

export function forwardFilters(fk: ForeignKey, columns: readonly string[], row: readonly Cell[]): QuickFilter[] | null {
  const values = valuesOf(fk.columns, columns, row);
  return values && fk.ref_columns.map((column, i) => ({ column, op: 'eq', value: values[i] ?? null }));
}

export function backwardFilters(fk: ForeignKey, columns: readonly string[], row: readonly Cell[]): QuickFilter[] | null {
  const values = valuesOf(fk.ref_columns, columns, row);
  return values && fk.columns.map((column, i) => ({ column, op: 'eq', value: values[i] ?? null }));
}

export const filterText = (quick: readonly QuickFilter[]) => quick.map((q) => `${q.column} = ${q.value ?? 'NULL'}`).join(', ');

const TEXTY = /char|text|string|clob|name|citext|enum/i;
const NAMEY = /^(nome|name|ragione_sociale|denominazione|descrizione|description|titolo|title|label|etichetta|cognome|surname|codice|code|numero|number|email)$/i;

export function labelColumns(columns: readonly ColumnInfo[], skip: readonly string[], max = 2): string[] {
  const free = columns.filter((c) => !skip.includes(c.name));
  const named = free.filter((c) => NAMEY.test(c.name));
  const texty = free.filter((c) => TEXTY.test(c.type_name) && !named.includes(c));
  const rest = free.filter((c) => !named.includes(c) && !texty.includes(c));
  return [...named, ...texty, ...rest].slice(0, max).map((c) => c.name);
}

export interface Crumb {
  namespace: string;
  name: string;
  where: string;
  quick: QuickFilter[];
  order: { column: string; descending: boolean }[];
  offset: number;
  focus?: { r: number; c: number };
}

export const crumbLabel = (c: Pick<Crumb, 'name' | 'quick'>) => (c.quick.length ? `${c.name} (${filterText(c.quick)})` : c.name);
