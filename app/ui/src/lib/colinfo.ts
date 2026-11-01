import type { ColumnInfo, TableDesign } from './bindings';
import type { Catalog, RelEntry } from './complete/catalog';

export interface ColumnFacts {
  ns: string;
  table: string;
  column: ColumnInfo;
  points: string[];
  pointedBy: string[];
  indexes: { name: string; unique: boolean; columns: string[] }[];
}

export interface ColumnExtra {
  type: string;
  default: string | null;
  comment: string | null;
  identity: boolean;
}

export type Describe = (ns: string, table: string) => Promise<TableDesign | null>;

const same = (a: string, b: string) => a.toLowerCase() === b.toLowerCase();

export function factsOf(catalog: Catalog, rel: RelEntry, name: string): ColumnFacts | null {
  const column = catalog.columns(rel).find((c) => same(c.name, name));
  if (!column) return null;
  const label = (r: RelEntry) => (r.home || r.ns === rel.ns ? r.name : `${r.ns}.${r.name}`);
  const points: string[] = [];
  const pointedBy: string[] = [];
  for (const l of catalog.linksOf(rel)) {
    if (l.child === rel) {
      const i = l.fk.columns.findIndex((c) => same(c, name));
      if (i >= 0) points.push(`${label(l.parent)}.${l.fk.ref_columns[i] ?? '?'}`);
    }
    if (l.parent === rel) {
      const i = l.fk.ref_columns.findIndex((c) => same(c, name));
      if (i >= 0) pointedBy.push(`${label(l.child)}.${l.fk.columns[i] ?? '?'}`);
    }
  }
  const indexes = rel.rel.indexes.filter((x) => x.columns.some((c) => same(c, name))).map((x) => ({ name: x.name, unique: x.unique, columns: x.columns }));
  return { ns: rel.ns, table: rel.name, column, points, pointedBy, indexes };
}

export function extraOf(design: TableDesign | null, name: string): ColumnExtra | null {
  const c = design?.columns.find((x) => same(x.name, name));
  if (!c) return null;
  return { type: c.type_name, default: c.default ?? null, comment: c.comment ?? null, identity: c.identity };
}

const cache = new Map<string, Promise<TableDesign | null>>();

export function describer(sessionId: string, schemaStamp: object | null, read: (sid: string, ns: string, table: string) => Promise<TableDesign>): Describe {
  const stamp = schemaStamp ? stampOf(schemaStamp) : 0;
  return (ns, table) => {
    const k = `${sessionId}\u0000${stamp}\u0000${ns}\u0000${table}`;
    let hit = cache.get(k);
    if (!hit) {
      if (cache.size > 200) cache.clear();
      hit = read(sessionId, ns, table).catch(() => null);
      cache.set(k, hit);
    }
    return hit;
  };
}

const stamps = new WeakMap<object, number>();
let next = 1;
function stampOf(o: object): number {
  let s = stamps.get(o);
  if (!s) {
    s = next++;
    stamps.set(o, s);
  }
  return s;
}
