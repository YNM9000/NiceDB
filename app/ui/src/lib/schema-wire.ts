import type { ColumnInfo, FkAction, ForeignKey, Index, PackedRelation_Serialize as PackedRelation, PackedSchema_Serialize as PackedSchema, Relation_Serialize as Relation, Schema_Serialize as Schema } from './bindings';

const ESCAPED = /\\(.)/g;
const unescape = (s: string) => s.replace(ESCAPED, (_, c: string) => (c === 't' ? '\t' : c === 'n' ? '\n' : c));

function split(s: string, sep: string): string[] {
  const out: string[] = [];
  let start = 0;
  for (let i = 0; i < s.length; i++) {
    const c = s[i];
    if (c === '\\') i++;
    else if (c === sep) {
      out.push(s.slice(start, i));
      start = i + 1;
    }
  }
  out.push(s.slice(start));
  return out;
}

function rows(packed: string): string[][] {
  if (!packed) return [];
  if (!packed.includes('\\')) return packed.split('\n').map((l) => l.split('\t'));
  return split(packed, '\n').map((l) => split(l, '\t').map(unescape));
}

export function unpackColumns(packed: string): ColumnInfo[] {
  return rows(packed).map(([name = '', type_name = '', flags = '0']) => {
    const f = flags.charCodeAt(0) - 48;
    return { name, type_name, nullable: (f & 1) === 1, primary_key: (f & 2) === 2 };
  });
}

function unpackIndexes(packed: string): Index[] {
  return rows(packed).map(([name = '', unique, ...columns]) => ({ name, columns, unique: unique === '1' }));
}

function unpackForeignKeys(packed: string): ForeignKey[] {
  return rows(packed).map(([name = '', ref_namespace = '', ref_table = '', on_delete, on_update, ...pairs]) => ({
    name,
    columns: pairs.filter((_, i) => i % 2 === 0),
    ref_namespace,
    ref_table,
    ref_columns: pairs.filter((_, i) => i % 2 === 1),
    on_delete: (on_delete ?? 'no_action') as FkAction,
    on_update: (on_update ?? 'no_action') as FkAction,
  }));
}

const PACKED = Symbol('packed');

interface Lazy {
  [PACKED]: [string | null, string | null, string | null];
}

function accessor<T>(slot: 0 | 1 | 2, key: string, read: (s: string) => T[]): PropertyDescriptor {
  return {
    enumerable: true,
    configurable: true,
    get(this: Lazy) {
      const value = read(this[PACKED][slot] ?? '');
      this[PACKED][slot] = null;
      Object.defineProperty(this, key, { value, writable: true, enumerable: true, configurable: true });
      return value;
    },
    set(this: Lazy, value: T[]) {
      this[PACKED][slot] = null;
      Object.defineProperty(this, key, { value, writable: true, enumerable: true, configurable: true });
    },
  };
}

const ACCESSORS: PropertyDescriptorMap = {
  columns: accessor(0, 'columns', unpackColumns),
  indexes: accessor(1, 'indexes', unpackIndexes),
  foreign_keys: accessor(2, 'foreign_keys', unpackForeignKeys),
};

function relation([name, columns, indexes, foreignKeys, rows]: PackedRelation): Relation {
  const r = { name, estimated_rows: rows, [PACKED]: [columns, indexes, foreignKeys] };
  return Object.defineProperties(r, ACCESSORS) as unknown as Relation;
}

export function unpackSchema(p: PackedSchema): Schema {
  return {
    databases: p.databases,
    current_database: p.current_database,
    namespaces: p.namespaces.map(({ details, materialized_views, dictionaries, aliases, data_streams, ...n }) => {
      const read = (list: PackedRelation[] | undefined) =>
        (list ?? []).map((packed) => {
          const r = relation(packed);
          const detail = details?.[r.name];
          if (detail !== undefined) r.detail = detail;
          return r;
        });
      return {
        ...n,
        tables: read(n.tables),
        views: read(n.views),
        ...(materialized_views?.length ? { materialized_views: read(materialized_views) } : {}),
        ...(dictionaries?.length ? { dictionaries: read(dictionaries) } : {}),
        ...(aliases?.length ? { aliases: read(aliases) } : {}),
        ...(data_streams?.length ? { data_streams: read(data_streams) } : {}),
      };
    }),
  };
}

const esc = (s: string) => (/[\\\t\n]/.test(s) ? s.replace(/[\\\t\n]/g, (c) => (c === '\t' ? '\\t' : c === '\n' ? '\\n' : '\\\\')) : s);
const lines = (list: string[][]) => list.map((l) => l.map(esc).join('\t')).join('\n');

export function packRelation(r: Relation): PackedRelation {
  return [
    r.name,
    lines(r.columns.map((c) => [c.name, c.type_name, String((c.nullable ? 1 : 0) + (c.primary_key ? 2 : 0))])),
    lines(r.indexes.map((i) => [i.name, i.unique ? '1' : '0', ...i.columns])),
    lines(r.foreign_keys.map((k) => [k.name, k.ref_namespace, k.ref_table, k.on_delete, k.on_update, ...k.columns.flatMap((c, i) => [c, k.ref_columns[i] ?? ''])])),
    r.estimated_rows,
  ];
}

export function packSchema(s: Schema): PackedSchema {
  return {
    databases: s.databases,
    current_database: s.current_database,
    namespaces: s.namespaces.map(({ materialized_views, dictionaries, aliases, data_streams, ...n }) => {
      const all = [...n.tables, ...n.views, ...(materialized_views ?? []), ...(dictionaries ?? []), ...(aliases ?? []), ...(data_streams ?? [])];
      const details = Object.fromEntries(all.flatMap((r) => (r.detail ? [[r.name, r.detail]] : [])));
      return {
        ...n,
        tables: n.tables.map(packRelation),
        views: n.views.map(packRelation),
        ...(materialized_views?.length ? { materialized_views: materialized_views.map(packRelation) } : {}),
        ...(aliases?.length ? { aliases: aliases.map(packRelation) } : {}),
        ...(data_streams?.length ? { data_streams: data_streams.map(packRelation) } : {}),
        ...(dictionaries?.length ? { dictionaries: dictionaries.map(packRelation) } : {}),
        ...(Object.keys(details).length ? { details } : {}),
      };
    }),
  };
}
