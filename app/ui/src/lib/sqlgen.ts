import { m } from '../paraglide/messages';
import type { ColumnInfo, IdentityColumn, Dependency_Serialize as Dependency, Dialect, GenRequest, Generator, Relation_Serialize as Relation } from './bindings';
import { isImplicit } from './sqlname';

export type { Dependency, Generator };

export interface GenSubject {
  dialect: Dialect;
  database: string;
  namespace: string;
  name: string;
  view: boolean;
  columns: ColumnInfo[] | null;
  key: string[];
}

export function subjectOf(dialect: Dialect, database: string, namespace: string, rel: Relation | null, name: string, view: boolean): GenSubject {
  const columns = rel?.columns ?? null;
  let key = columns?.filter((c) => c.primary_key).map((c) => c.name) ?? [];
  if (!key.length && rel && !view) {
    const unique = rel.indexes.find((ix) => ix.unique && ix.columns.length > 0 && ix.columns.every((n) => rel.columns.find((c) => c.name === n)?.nullable === false));
    key = unique?.columns ?? [];
  }
  return { dialect, database, namespace, name, view, columns, key: view ? [] : key };
}

export const ROW_WRITES: readonly Generator[] = ['update', 'delete', 'upsert'];

export function generatorsOf(s: GenSubject): { generator: Generator; label: string; hint?: string; disabled: boolean; danger?: boolean }[] {
  const noColumns = !s.columns?.length;
  const keyed = s.key.length > 0;
  const keyHint = keyed ? m.gen_by({ columns: s.key.join(', ') }) : m.gen_no_key();
  const out: ReturnType<typeof generatorsOf> = [{ generator: 'select', label: 'SELECT', disabled: false }];
  if (!s.view) {
    out.push(
      { generator: 'insert', label: 'INSERT', disabled: noColumns },
      { generator: 'update', label: m.gen_update(), hint: keyHint, disabled: noColumns || !keyed },
      { generator: 'delete', label: m.gen_delete(), hint: keyHint, disabled: noColumns || !keyed },
      { generator: 'upsert', label: upsertLabel(s.dialect), hint: keyed ? undefined : m.gen_no_key(), disabled: noColumns || !keyed },
    );
  }
  out.push({ generator: 'copy', label: s.view ? m.gen_copy_view() : m.gen_copy_table(), hint: m.gen_copy_name({ name: s.name }), disabled: false });
  out.push({ generator: 'drop', label: s.view ? 'DROP VIEW' : 'DROP TABLE', disabled: false, danger: true });
  return out;
}

export function upsertLabel(d: Dialect): string {
  if (d === 'mysql') return 'INSERT … ON DUPLICATE KEY';
  if (d === 'mssql') return 'MERGE';
  if (d === 'clickhouse') return m.gen_upsert_replacing();
  return 'INSERT … ON CONFLICT';
}

export function requestOf(s: GenSubject, generator: Generator, identity: readonly IdentityColumn[] = []): GenRequest {
  const implicit = isImplicit(s.dialect, s.namespace, s.database);
  const key = new Set(s.key);
  const numbered = new Map(identity.map((c) => [c.name, c.identity]));
  return {
    table: { namespace: implicit ? '' : s.namespace, name: s.name },
    view: s.view,
    columns: (s.columns ?? []).map((c) => ({ name: c.name, primary_key: key.has(c.name), type_name: c.type_name, not_null: !c.nullable, identity: numbered.get(c.name) ?? null })),
    generator,
    copy_name: m.gen_copy_name({ name: s.name }),
  };
}

export type DepGroup = { title: string; items: Dependency[] };

export function groupDeps(deps: Dependency[], view: boolean): DepGroup[] {
  const order = ['table', 'view', 'routine', 'trigger', 'sequence'];
  const sort = (a: Dependency, b: Dependency) => order.indexOf(a.kind) - order.indexOf(b.kind) || a.namespace.localeCompare(b.namespace) || a.name.localeCompare(b.name);
  const uses = deps.filter((d) => d.direction === 'uses').sort(sort);
  const usedBy = deps.filter((d) => d.direction === 'used_by').sort(sort);
  return [
    { title: view ? m.deps_view_uses() : m.deps_table_uses(), items: uses },
    { title: view ? m.deps_view_used_by() : m.deps_table_used_by(), items: usedBy },
  ];
}

export function viaLabel(d: Dependency): string {
  switch (d.via) {
    case 'foreign_key':
      return d.detail ? m.deps_via_fk_named({ name: d.detail }) : m.deps_via_fk();
    case 'trigger':
      return d.detail ? m.deps_via_trigger_fn({ name: d.detail }) : m.deps_via_trigger();
    case 'owned':
      return m.deps_via_owned();
    default:
      return m.deps_via_definition();
  }
}

export const readFromText = (d: Dialect) => d === 'sqlite' || d === 'duckdb' || d === 'mysql';
