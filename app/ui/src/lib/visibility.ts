import type { Schema, Visibility } from './api';

const HIDDEN_DATABASES = new Set(['template0', 'template1', 'information_schema', 'performance_schema', 'mysql', 'sys', 'master', 'model', 'msdb', 'tempdb']);
const HIDDEN_SCHEMAS = new Set(['information_schema', 'pg_catalog', 'pg_toast', 'sys', 'crdb_internal', 'performance_schema', 'mysql']);

export const emptyVisibility = (): Visibility => ({ databases: {}, schemas: {} });

export const databaseShown = (v: Visibility, name: string) => v.databases[name] ?? !HIDDEN_DATABASES.has(name.toLowerCase());

export const schemaShown = (v: Visibility, database: string, name: string) => v.schemas[database]?.[name] ?? !HIDDEN_SCHEMAS.has(name.toLowerCase());

export function shownDatabases(v: Visibility, all: string[], current: string, showAll = false): string[] {
  return all.filter((d) => showAll || d === current || databaseShown(v, d));
}

export function visibleSchema(schema: Schema, v: Visibility, showAll = false): Schema {
  if (showAll) return schema;
  const namespaces = schema.namespaces.filter((n) => schemaShown(v, schema.current_database, n.name));
  return namespaces.length === schema.namespaces.length ? schema : { ...schema, namespaces };
}

export function setDatabase(v: Visibility, name: string, shown: boolean): Visibility {
  return { ...v, databases: { ...v.databases, [name]: shown } };
}

export function setSchema(v: Visibility, database: string, name: string, shown: boolean): Visibility {
  return { ...v, schemas: { ...v.schemas, [database]: { ...v.schemas[database], [name]: shown } } };
}

export function onlyUsed(v: Visibility, schema: Schema, used: Iterable<string>): Visibility {
  const keep = new Set([...used, schema.current_database]);
  const databases = Object.fromEntries(schema.databases.map((d) => [d, keep.has(d)]));
  const inner = Object.fromEntries(schema.namespaces.map((n) => [n.name, n.tables.length + n.views.length > 0 && !HIDDEN_SCHEMAS.has(n.name.toLowerCase())]));
  return { databases, schemas: { ...v.schemas, [schema.current_database]: inner } };
}
