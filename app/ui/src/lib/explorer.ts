import type { Activity, Principals, Schema } from './api';
import type { Flavor, RelationTag, Relation_Serialize as Relation } from './bindings';

export type GroupKind = 'tables' | 'views' | 'functions' | 'sequences' | 'types' | 'triggers' | 'materialized_views' | 'dictionaries' | 'aliases' | 'data_streams' | 'packages' | 'synonyms';

export const RELATION_GROUPS: readonly GroupKind[] = ['tables', 'views', 'materialized_views', 'dictionaries', 'aliases', 'data_streams'];

export const isRelationGroup = (g: GroupKind | null): boolean => g !== null && RELATION_GROUPS.includes(g);

export const isViewGroup = (g: GroupKind | null): boolean => g === 'views' || g === 'materialized_views' || g === 'dictionaries' || g === 'aliases' || g === 'data_streams';
export type ServerGroup = 'users' | 'sessions';

export interface Shape {
  databases: boolean;
  schemas: 'always' | 'multiple';
  groups: readonly GroupKind[];
  server: boolean;
}


export type NodeKind = 'databases' | 'database' | 'schemas' | 'platform' | 'schema' | 'group' | 'relation' | 'routine' | 'object' | 'column' | 'index' | 'server' | 'principal' | 'session' | 'note';

export interface NodePath {
  database: string | null;
  namespace: string | null;
  group: GroupKind | null;
  server: ServerGroup | null;
  name: string | null;
  id: string | null;
}

export interface ExNode {
  key: string;
  kind: NodeKind;
  label: string;
  depth: number;
  expandable: boolean;
  open: boolean;
  count: number | null;
  detail: string | null;
  active: boolean;
  dim: boolean;
  busy: boolean;
  failed: boolean;
  path: NodePath;
  tags?: readonly RelationTag[];
  platform?: Flavor;
}

export interface Loaded<T> {
  status: 'idle' | 'loading' | 'ready' | 'failed';
  data: T | null;
  error: string | null;
}

export const idle = <T>(): Loaded<T> => ({ status: 'idle', data: null, error: null });

export interface TreeInput {
  connection: string;
  shape: Shape;
  databases: string[];
  current: string | null;
  loaded: (database: string) => Loaded<Schema>;
  isOpen: (key: string) => boolean;
  filter: string;
  databaseDim: (database: string) => boolean;
  schemaShown: (database: string, namespace: string) => boolean;
  schemaDim: (database: string, namespace: string) => boolean;
  users: Loaded<Principals> | null;
  sessions: Loaded<Activity> | null;
  unsupported: { users: boolean; sessions: boolean };
  catalogCount?: (database: string) => number | null;
  pending?: (database: string, namespace: string) => { relations: number; status: 'idle' | 'loading' | 'failed'; error: string | null } | null;
}

const SEP = '\u001f';

export const keyOf = (...parts: (string | null)[]) => parts.map((p) => p ?? '').join(SEP);

const empty = (): NodePath => ({ database: null, namespace: null, group: null, server: null, name: null, id: null });

type Seed = Partial<ExNode> & Pick<ExNode, 'key' | 'kind' | 'label' | 'depth'>;

const node = (n: Seed): ExNode => ({ expandable: false, open: false, count: null, detail: null, active: false, dim: false, busy: false, failed: false, path: empty(), ...n });

export const matches = (name: string, filter: string) => !filter || name.toLowerCase().includes(filter);

export function relationsOf(schema: Schema, namespace: string, group: GroupKind): Relation[] {
  const ns = schema.namespaces.find((n) => n.name === namespace);
  if (!ns) return [];
  switch (group) {
    case 'tables':
      return ns.tables;
    case 'views':
      return ns.views;
    case 'materialized_views':
      return ns.materialized_views ?? [];
    case 'dictionaries':
      return ns.dictionaries ?? [];
    case 'aliases':
      return ns.aliases ?? [];
    case 'data_streams':
      return ns.data_streams ?? [];
    default:
      return [];
  }
}

interface Entry {
  name: string;
  detail: string | null;
  sub: string;
}

function entries(ns: Schema['namespaces'][number], group: GroupKind): Entry[] {
  switch (group) {
    case 'functions':
      return ns.functions.map((f) => ({ name: f.name, detail: `(${f.signature})`, sub: f.signature }));
    case 'sequences':
      return ns.sequences.map((o) => ({ name: o.name, detail: o.detail, sub: '' }));
    case 'types':
      return ns.types.map((o) => ({ name: o.name, detail: o.detail, sub: '' }));
    case 'triggers':
      return ns.triggers.map((o) => ({ name: o.name, detail: o.detail, sub: '' }));
    case 'packages':
      return (ns.packages ?? []).map((o) => ({ name: o.name, detail: null, sub: '' }));
    case 'synonyms':
      return (ns.synonyms ?? []).map((o) => ({ name: o.name, detail: o.detail, sub: '' }));
    default:
      return [];
  }
}

const sizeOf = (ns: Schema['namespaces'][number], group: GroupKind): number => (isRelationGroup(group) ? relationsOf({ databases: [], current_database: '', namespaces: [ns] }, ns.name, group).length : entries(ns, group).length);

function groupNodes(input: TreeInput, database: string, ns: Schema['namespaces'][number], depth: number, out: ExNode[]) {
  const q = input.filter;
  for (const group of input.shape.groups) {
    const total = sizeOf(ns, group);
    if (total === 0) continue;
    const key = keyOf(input.connection, 'db', database, 'ns', ns.name, 'g', group);
    const path: NodePath = { ...empty(), database, namespace: ns.name, group };
    const list = isRelationGroup(group) ? relationsOf({ databases: [], current_database: database, namespaces: [ns] }, ns.name, group) : null;
    const kept = list ? list.filter((r) => matches(r.name, q)) : entries(ns, group).filter((e) => matches(e.name, q));
    if (q && kept.length === 0) continue;
    const open = q ? true : input.isOpen(key);
    out.push(node({ key, kind: 'group', label: group, depth, expandable: true, open, count: q ? kept.length : total, path }));
    if (!open) continue;
    if (list) {
      for (const rel of kept as Relation[]) relationNodes(input, database, ns.name, group, rel, depth + 1, out);
    } else {
      for (const e of kept as Entry[]) {
        out.push(node({ key: keyOf(key, e.name, e.sub), kind: group === 'functions' ? 'routine' : 'object', label: e.name, depth: depth + 1, detail: e.detail, path: { ...path, name: e.name } }));
      }
    }
  }
}

function relationNodes(input: TreeInput, database: string, namespace: string, group: GroupKind, rel: Relation, depth: number, out: ExNode[]) {
  const key = keyOf(input.connection, 'db', database, 'ns', namespace, 'g', group, rel.name);
  const path: NodePath = { ...empty(), database, namespace, group, name: rel.name };
  const open = input.isOpen(key);
  const tags = input.loaded(database).data?.namespaces.find((n) => n.name === namespace)?.tags?.[rel.name];
  out.push(node({ key, kind: 'relation', label: rel.name, depth, expandable: true, open, count: rel.estimated_rows, detail: rel.detail ?? null, path, ...(tags?.length ? { tags } : {}) }));
  if (!open) return;
  for (const c of rel.columns) out.push(node({ key: keyOf(key, 'c', c.name), kind: 'column', label: c.name, depth: depth + 1, detail: `${c.type_name}${c.nullable ? '' : ' ·'}`, active: c.primary_key, path: { ...path, id: c.name } }));
  for (const i of rel.indexes) out.push(node({ key: keyOf(key, 'i', i.name), kind: 'index', label: i.name, depth: depth + 1, detail: `${i.unique ? 'unique ' : ''}(${i.columns.join(', ')})`, path: { ...path, id: i.name } }));
}

function schemaNodes(input: TreeInput, database: string, depth: number, out: ExNode[]) {
  const state = input.loaded(database);
  const key = keyOf(input.connection, 'db', database);
  if (state.status === 'loading' || state.status === 'idle') {
    out.push(node({ key: keyOf(key, 'wait'), kind: 'note', label: 'loading', depth, busy: true }));
    return;
  }
  if (state.status === 'failed' || !state.data) {
    out.push(node({ key: keyOf(key, 'fail'), kind: 'note', label: state.error ?? '', depth, failed: true }));
    return;
  }
  const spaces = state.data.namespaces.filter((n) => input.schemaShown(database, n.name));
  const levelled = input.shape.schemas === 'always' || spaces.length > 1;
  if (!levelled) {
    const only = spaces[0];
    if (only) groupNodes(input, database, only, depth, out);
    return;
  }
  const holder = keyOf(key, 'schemas');
  const open = input.filter ? true : input.isOpen(holder);
  out.push(node({ key: holder, kind: 'schemas', label: 'schemas', depth, expandable: true, open, count: spaces.length, path: { ...empty(), database } }));
  if (!open) return;
  const one = (ns: Schema['namespaces'][number], at: number) => {
    const nsKey = keyOf(key, 'ns', ns.name);
    const wait = input.pending?.(database, ns.name) ?? null;
    const nsOpen = input.filter && !wait ? true : input.isOpen(nsKey);
    const before = out.length;
    out.push(node({ key: nsKey, kind: 'schema', label: ns.name, depth: at, expandable: true, open: nsOpen, count: wait ? wait.relations : ns.tables.length + ns.views.length, dim: input.schemaDim(database, ns.name), path: { ...empty(), database, namespace: ns.name } }));
    if (nsOpen && wait && !input.filter) {
      const failed = wait.status === 'failed';
      out.push(node({ key: keyOf(nsKey, failed ? 'fail' : 'wait'), kind: 'note', label: failed ? (wait.error ?? '') : 'loading', depth: at + 1, busy: !failed, failed }));
    } else if (nsOpen && !wait) groupNodes(input, database, ns, at + 1, out);
    if (input.filter && out.length === before + 1) out.pop();
  };
  for (const ns of spaces) if (!ns.managed) one(ns, depth + 1);
  for (const platform of [...new Set(spaces.flatMap((n) => (n.managed ? [n.managed] : [])))]) {
    const owned = spaces.filter((n) => n.managed === platform);
    const pKey = keyOf(key, 'platform', platform);
    const pOpen = input.filter ? true : input.isOpen(pKey);
    const before = out.length;
    out.push(node({ key: pKey, kind: 'platform', label: platform, depth: depth + 1, expandable: true, open: pOpen, count: owned.length, platform, path: { ...empty(), database } }));
    if (pOpen) for (const ns of owned) one(ns, depth + 2);
    if (input.filter && out.length === before + 1) out.pop();
  }
}

function serverNodes(input: TreeInput, depth: number, out: ExNode[]) {
  const groups: { id: ServerGroup; state: Loaded<unknown> | null; off: boolean }[] = [
    { id: 'users', state: input.users, off: input.unsupported.users },
    { id: 'sessions', state: input.sessions, off: input.unsupported.sessions },
  ];
  for (const g of groups) {
    const key = keyOf(input.connection, 'srv', g.id);
    const open = input.isOpen(key);
    const state = g.state;
    const path: NodePath = { ...empty(), server: g.id };
    const principals = g.id === 'users' ? input.users?.data : null;
    const sessions = g.id === 'sessions' ? input.sessions?.data : null;
    const count = principals ? principals.principals.length : sessions ? sessions.sessions.length : null;
    out.push(node({ key, kind: 'server', label: g.id, depth, expandable: true, open, count, busy: state?.status === 'loading', path }));
    if (!open) continue;
    if (state?.status === 'failed') {
      out.push(node({ key: keyOf(key, 'fail'), kind: 'note', label: state.error ?? '', depth: depth + 1, failed: true, detail: g.off ? 'unsupported' : null }));
    } else if (!state || state.status !== 'ready') {
      out.push(node({ key: keyOf(key, 'wait'), kind: 'note', label: 'loading', depth: depth + 1, busy: true }));
    } else if (principals) {
      for (const p of principals.principals) {
        const name = p.host ? `${p.name}@${p.host}` : p.name;
        out.push(node({ key: keyOf(key, name), kind: 'principal', label: name, depth: depth + 1, detail: p.member_of.length ? p.member_of.join(', ') : null, path: { ...path, name } }));
      }
    } else if (sessions) {
      for (const s of sessions.sessions) {
        const label = s.user ? `${s.id} · ${s.user}` : s.id;
        out.push(node({ key: keyOf(key, s.id), kind: 'session', label, depth: depth + 1, detail: s.state, path: { ...path, id: s.id, name: s.query ?? null } }));
      }
    }
  }
}

export function buildNodes(input: TreeInput): ExNode[] {
  const out: ExNode[] = [];
  if (input.shape.databases) {
    const holder = keyOf(input.connection, 'dbs');
    const open = input.isOpen(holder);
    out.push(node({ key: holder, kind: 'databases', label: 'databases', depth: 0, expandable: true, open, count: input.databases.length }));
    if (open) {
      for (const database of input.databases) {
        const key = keyOf(input.connection, 'db', database);
        const dbOpen = input.isOpen(key);
        const count = input.loaded(database).data ? null : (input.catalogCount?.(database) ?? null);
        out.push(node({ key, kind: 'database', label: database, depth: 1, expandable: true, open: dbOpen, count, active: database === input.current, dim: input.databaseDim(database), path: { ...empty(), database } }));
        if (dbOpen) schemaNodes(input, database, 2, out);
      }
    }
  } else if (input.current !== null) {
    schemaNodes(input, input.current, 0, out);
  }
  if (input.shape.server) serverNodes(input, 0, out);
  return out;
}

export const parentIndex = (nodes: readonly ExNode[], i: number): number => {
  const depth = nodes[i]?.depth ?? 0;
  for (let j = i - 1; j >= 0; j--) if ((nodes[j]?.depth ?? 0) < depth) return j;
  return -1;
};

export function typeAhead(nodes: readonly ExNode[], from: number, prefix: string): number {
  const p = prefix.toLowerCase();
  if (!p || nodes.length === 0) return -1;
  const cycle = p.length > 1 && [...p].every((c) => c === p[0]);
  const needle = cycle ? p[0] ?? '' : p;
  const start = p.length === 1 || cycle ? from + 1 : from;
  for (let step = 0; step < nodes.length; step++) {
    const i = (start + step + nodes.length) % nodes.length;
    if (nodes[i]?.label.toLowerCase().startsWith(needle) && nodes[i]?.kind !== 'note') return i;
  }
  return -1;
}

export interface Window {
  start: number;
  end: number;
}

export function windowOf(total: number, rowPx: number, above: number, below: number, over = 12): Window {
  if (total <= 0 || rowPx <= 0) return { start: 0, end: Math.max(0, total) };
  const start = Math.min(total, Math.max(0, Math.floor(above / rowPx) - over));
  const end = Math.max(start, Math.min(total, Math.ceil(below / rowPx) + over));
  return { start, end };
}

export const WINDOW_FROM = 120;
