import type { Schema } from '../api';
import type { ColumnInfo, ForeignKey } from '../bindings';
import { DEFAULT_NAMESPACES } from './dialects';

type Namespace = Schema['namespaces'][number];
type Relation = Namespace['tables'][number];

export interface RelEntry {
  ns: string;
  name: string;
  kind: 'table' | 'view';
  rel: Relation;
  home: boolean;
}

export interface Link {
  child: RelEntry;
  parent: RelEntry;
  fk: ForeignKey;
}

const key = (s: string) => s.toLowerCase();

export class Catalog {
  readonly databases: string[];
  readonly current: string;
  private readonly namespaces: Namespace[];
  private readonly homes: Set<string>;
  private readonly path: string[] | null;
  private rels: RelEntry[] | null = null;
  private byName: Map<string, RelEntry[]> | null = null;
  private byNs: Map<string, Namespace> | null = null;
  private columnSets = new WeakMap<Relation, Map<string, ColumnInfo>>();
  private links: Map<RelEntry, Link[]> | null = null;

  constructor(schema: Schema, searchPath: readonly string[] | null = null) {
    this.databases = schema.databases;
    this.current = schema.current_database;
    this.namespaces = schema.namespaces;
    this.path = searchPath?.length ? searchPath.map(key) : null;
    if (this.path) {
      this.homes = new Set(this.path);
      return;
    }
    const homes = schema.namespaces.filter((n) => DEFAULT_NAMESPACES.has(key(n.name))).map((n) => key(n.name));
    const current = schema.namespaces.filter((n) => key(n.name) === key(schema.current_database)).map((n) => key(n.name));
    this.homes = new Set(homes.length ? homes : current.length ? current : schema.namespaces.length ? [key(schema.namespaces[0]?.name ?? '')] : []);
  }

  get empty(): boolean {
    return this.namespaces.every((n) => n.tables.length === 0 && n.views.length === 0);
  }

  relations(): RelEntry[] {
    if (!this.rels) {
      const out: RelEntry[] = [];
      for (const ns of this.namespaces) {
        const home = this.homes.has(key(ns.name));
        for (const rel of ns.tables) out.push({ ns: ns.name, name: rel.name, kind: 'table', rel, home });
        for (const rel of ns.views) out.push({ ns: ns.name, name: rel.name, kind: 'view', rel, home });
      }
      this.rels = out;
      if (this.path) for (const r of out) if (r.home) r.home = this.find([r.name])[0]?.ns === r.ns;
    }
    return this.rels;
  }

  private index(): Map<string, RelEntry[]> {
    if (!this.byName) {
      const m = new Map<string, RelEntry[]>();
      for (const r of this.relations()) {
        const k = key(r.name);
        const list = m.get(k);
        if (list) list.push(r);
        else m.set(k, [r]);
      }
      this.byName = m;
    }
    return this.byName;
  }

  namespace(name: string): Namespace | undefined {
    if (!this.byNs) this.byNs = new Map(this.namespaces.map((n) => [key(n.name), n]));
    return this.byNs.get(key(name));
  }

  namespaceNames(): string[] {
    return this.namespaces.map((n) => n.name);
  }

  isHome(ns: string): boolean {
    return this.homes.has(key(ns));
  }

  isDatabase(name: string): boolean {
    return this.databases.some((d) => key(d) === key(name));
  }

  find(parts: readonly string[]): RelEntry[] {
    const name = parts.at(-1);
    if (!name) return [];
    const all = this.index().get(key(name)) ?? [];
    if (parts.length === 1 && this.path) {
      for (const p of this.path) {
        const hit = all.filter((r) => key(r.ns) === p);
        if (hit.length) return hit;
      }
      return [];
    }
    if (parts.length === 1) {
      const home = all.filter((r) => r.home);
      return home.length ? home : all;
    }
    const ns = parts.at(-2) ?? '';
    const inNs = all.filter((r) => key(r.ns) === key(ns));
    if (inNs.length) {
      const db = parts.length > 2 ? parts.at(-3) : null;
      return !db || key(db) === key(this.current) ? inNs : [];
    }
    return parts.length === 2 && key(ns) === key(this.current) ? all.filter((r) => r.home) : [];
  }

  knowsQualifier(parts: readonly string[]): boolean {
    if (parts.length === 1) return !!this.namespace(parts[0] ?? '') || key(parts[0] ?? '') === key(this.current);
    return key(parts[0] ?? '') === key(this.current) && !!this.namespace(parts[1] ?? '');
  }

  columns(r: RelEntry): ColumnInfo[] {
    return r.rel.columns;
  }

  hasColumn(r: RelEntry, name: string): boolean {
    let set = this.columnSets.get(r.rel);
    if (!set) {
      set = new Map(r.rel.columns.map((c) => [key(c.name), c]));
      this.columnSets.set(r.rel, set);
    }
    return set.has(key(name));
  }

  linksOf(r: RelEntry): Link[] {
    if (!this.links) {
      const m = new Map<RelEntry, Link[]>();
      const add = (at: RelEntry, l: Link) => {
        const list = m.get(at);
        if (list) list.push(l);
        else m.set(at, [l]);
      };
      for (const child of this.relations()) {
        for (const fk of child.rel.foreign_keys) {
          const parent = this.find([fk.ref_namespace, fk.ref_table])[0] ?? this.find([fk.ref_table])[0];
          if (!parent) continue;
          const l = { child, parent, fk };
          add(child, l);
          if (parent !== child) add(parent, l);
        }
      }
      this.links = m;
    }
    return this.links.get(r) ?? [];
  }

  schemasOf(name: string): string[] {
    return (this.index().get(key(name)) ?? []).map((r) => r.ns);
  }

  get strict(): boolean {
    return this.path !== null;
  }

  functions(): { ns: string; name: string; signature: string; home: boolean }[] {
    return this.namespaces.flatMap((n) => n.functions.map((f) => ({ ns: n.name, name: f.name, signature: f.signature, home: this.homes.has(key(n.name)) })));
  }
}

const catalogs = new WeakMap<Schema, Map<string, Catalog>>();

export function catalogOf(schema: Schema | null, searchPath: readonly string[] | null = null): Catalog | null {
  if (!schema) return null;
  let byPath = catalogs.get(schema);
  if (!byPath) {
    byPath = new Map();
    catalogs.set(schema, byPath);
  }
  const k = searchPath?.join('\u0000') ?? '';
  let c = byPath.get(k);
  if (!c) {
    c = new Catalog(schema, searchPath);
    byPath.set(k, c);
  }
  return c;
}
