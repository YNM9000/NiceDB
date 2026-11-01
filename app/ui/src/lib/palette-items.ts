import type { Activity, Mark, Principals, Schema } from './api';
import { drain, prepare, rankSteps, type Candidate, type Prepared } from './fuzzy';

export type Mode = 'objects' | 'library' | 'commands';

export type ItemKind = 'table' | 'view' | 'column' | 'function' | 'sequence' | 'type' | 'trigger' | 'index' | 'database' | 'session' | 'user' | 'script' | 'note' | 'snippet' | 'connection' | 'space' | 'query' | 'command';

export const MODE_OF: Record<ItemKind, Mode> = {
  table: 'objects',
  view: 'objects',
  column: 'objects',
  function: 'objects',
  sequence: 'objects',
  type: 'objects',
  trigger: 'objects',
  index: 'objects',
  database: 'objects',
  session: 'objects',
  user: 'objects',
  script: 'library',
  note: 'library',
  snippet: 'library',
  connection: 'library',
  space: 'library',
  query: 'library',
  command: 'commands',
};

export interface PaletteItem extends Candidate {
  readonly key: string;
  readonly kind: ItemKind;
  readonly label: string;
  readonly where: string;
  readonly detail: string;
  readonly connectionId: string | null;
  readonly database: string | null;
  readonly namespace: string | null;
  readonly ref: string | null;
  readonly count: number | null;
}

export interface ItemInit {
  key: string;
  kind: ItemKind;
  label: string;
  where?: string;
  detail?: string;
  words?: string;
  connectionId?: string | null;
  database?: string | null;
  namespace?: string | null;
  ref?: string | null;
  count?: number | null;
}

export function item(init: ItemInit): PaletteItem {
  const where = init.where ?? '';
  return {
    key: init.key,
    kind: init.kind,
    label: init.label,
    where,
    detail: init.detail ?? '',
    connectionId: init.connectionId ?? null,
    database: init.database ?? null,
    namespace: init.namespace ?? null,
    ref: init.ref ?? null,
    count: init.count ?? null,
    name: prepare(init.label),
    context: prepare(`${where} ${init.words ?? ''}`),
  };
}

const SEP = '\u001f';

export const objectKey = (kind: ItemKind, connectionId: string, database: string, namespace: string, name: string, table: string | null = null) => ['obj', kind, connectionId, database, namespace, table ?? '', name].join(SEP);

export interface Words {
  table: string;
  view: string;
  column: string;
  function: string;
  sequence: string;
  type: string;
  trigger: string;
  index: string;
  unique: string;
  database: string;
  session: string;
  user: string;
}

export interface SchemaSource {
  connectionId: string;
  connection: string;
  schema: Schema;
  main: boolean;
  words: Words;
}

type SchemaKind = 'table' | 'view' | 'column' | 'function' | 'sequence' | 'type' | 'trigger' | 'index' | 'database';

class SchemaItem implements PaletteItem {
  constructor(
    readonly kind: SchemaKind,
    readonly label: string,
    readonly where: string,
    readonly detail: string,
    readonly connectionId: string,
    readonly database: string,
    readonly namespace: string | null,
    readonly ref: string | null,
    readonly count: number | null,
    readonly name: Prepared,
    readonly context: Prepared,
  ) {}

  get key(): string {
    const ns = this.namespace ?? '';
    if (this.kind === 'column' || this.kind === 'index') return objectKey(this.kind, this.connectionId, this.database, ns, this.label, this.ref);
    if (this.kind === 'function') return objectKey('function', this.connectionId, this.database, ns, `${this.label}(${this.ref ?? ''})`);
    return objectKey(this.kind, this.connectionId, this.database, ns, this.label);
  }
}

export function schemaItems(src: SchemaSource): PaletteItem[] {
  const build = buildSchemaItems(src);
  for (;;) {
    const step = build.next();
    if (step.done) return step.value;
  }
}

export function* buildSchemaItems(src: SchemaSource): Generator<void, PaletteItem[]> {
  const { connectionId: cid, connection, schema, words } = src;
  const db = schema.current_database;
  const out: PaletteItem[] = [];
  const single = schema.namespaces.length === 1;
  const names = new Map<string, Prepared>();
  const named = (label: string) => {
    let p = names.get(label);
    if (!p) names.set(label, (p = prepare(label)));
    return p;
  };
  for (const ns of schema.namespaces) {
    const place = [single && ns.name === 'public' ? '' : ns.name, db, connection].filter(Boolean).join(' · ');
    for (const [kind, list] of [['table', ns.tables], ['view', ns.views]] as const) {
      const context = prepare(`${place} ${words[kind]}`);
      for (const r of list) {
        out.push(new SchemaItem(kind, r.name, place, '', cid, db, ns.name, null, r.estimated_rows, named(r.name), context));
        const where = `${r.name} · ${place}`;
        const around = prepare(`${where} ${words.column}`);
        for (const c of r.columns) out.push(new SchemaItem('column', c.name, where, c.type_name, cid, db, ns.name, r.name, null, named(c.name), around));
        if (r.indexes.length) {
          const owned = prepare(`${where} ${words.index}`);
          for (const i of r.indexes) out.push(new SchemaItem('index', i.name, where, `${i.unique ? `${words.unique} ` : ''}(${i.columns.join(', ')})`, cid, db, ns.name, r.name, null, named(i.name), owned));
        }
        yield;
      }
    }
    const context = prepare(`${place} ${words.function}`);
    for (const f of ns.functions) out.push(new SchemaItem('function', f.name, place, `(${f.signature})`, cid, db, ns.name, f.signature, null, named(f.name), context));
    for (const [kind, list] of [['sequence', ns.sequences], ['type', ns.types], ['trigger', ns.triggers]] as const) {
      if (!list.length) continue;
      const near = prepare(`${place} ${words[kind]}`);
      for (const o of list) out.push(new SchemaItem(kind, o.name, place, o.detail ?? '', cid, db, ns.name, kind === 'trigger' ? o.detail : null, null, named(o.name), near));
    }
  }
  if (src.main && schema.databases.length > 1) {
    const context = prepare(`${connection} ${words.database}`);
    for (const d of schema.databases) {
      if (d === db) continue;
      out.push(new SchemaItem('database', d, connection, '', cid, d, null, null, null, named(d), context));
    }
  }
  return out;
}

export function serverItems(connectionId: string, connection: string, activity: Activity | null, principals: Principals | null, words: Words): PaletteItem[] {
  const out: PaletteItem[] = [];
  for (const s of activity?.sessions ?? []) {
    const query = (s.query ?? '').replace(/\s+/g, ' ').slice(0, 120);
    out.push(item({ key: ['srv', 'session', connectionId, s.id].join(SEP), kind: 'session', label: `${s.id}${s.user ? ` · ${s.user}` : ''}`, where: [query || s.state, connection].filter(Boolean).join(' · '), detail: s.database ?? '', words: words.session, connectionId, ref: s.id }));
  }
  for (const p of principals?.principals ?? []) {
    const name = p.host ? `${p.name}@${p.host}` : p.name;
    out.push(item({ key: ['srv', 'user', connectionId, name].join(SEP), kind: 'user', label: name, where: connection, words: words.user, connectionId, ref: p.name }));
  }
  return out;
}

export const markKey = (mark: Mark): string => {
  const t = mark.target;
  if (t.kind === 'item') return t.key;
  if (t.kind === 'table') return objectKey(t.view ? 'view' : 'table', t.connection_id, t.database, t.namespace, t.name);
  return `query${SEP}${mark.id}`;
};

export function markItem(mark: Mark, connection: string): PaletteItem | null {
  const t = mark.target;
  if (t.kind === 'table') {
    const kind = t.view ? 'view' : 'table';
    return item({ key: markKey(mark), kind, label: t.name, where: [t.namespace, t.database, connection].filter(Boolean).join(' · '), connectionId: t.connection_id, database: t.database, namespace: t.namespace });
  }
  if (t.kind === 'query') {
    const flat = t.sql.replace(/\s+/g, ' ').trim();
    return item({ key: markKey(mark), kind: 'query', label: mark.label, where: [connection, t.database].filter(Boolean).join(' · '), words: flat.slice(0, 200), connectionId: t.connection_id, database: t.database, ref: mark.id });
  }
  return null;
}

export function queryLabel(sql: string, width = 60): string {
  const line = sql
    .split('\n')
    .map((l) => l.trim())
    .find((l) => l && !l.startsWith('--'));
  const flat = (line ?? sql.trim()).replace(/\s+/g, ' ');
  return flat.length > width ? `${flat.slice(0, width - 1)}…` : flat;
}

export const KIND_BOOST: Record<ItemKind, number> = {
  table: 250,
  view: 230,
  database: 150,
  function: 120,
  sequence: 0,
  type: 0,
  trigger: 0,
  index: 0,
  user: 10,
  session: 10,
  column: 0,
  script: 30,
  query: 30,
  connection: 28,
  space: 20,
  snippet: 12,
  note: 8,
  command: 0,
};

export const touch = (recent: readonly string[], key: string, cap = 40) => [key, ...recent.filter((k) => k !== key)].slice(0, cap);

export function noteTitle(body: string, width = 80): string {
  const line = body
    .split('\n')
    .map((l) => l.trim())
    .find((l) => l && !l.startsWith('```'));
  const flat = (line ?? '')
    .replace(/^(#{1,6}\s+|[-*+]\s+|>\s*|\d+\.\s+)/, '')
    .replace(/(\*\*|__|\*|_|`)(.+?)\1/g, '$2')
    .replace(/\[([^\]]*)\]\([^)]*\)/g, '$1')
    .trim();
  return flat.length > width ? `${flat.slice(0, width - 1)}…` : flat;
}

export interface Section {
  id: 'pinned' | 'bookmarks' | 'recent' | 'results' | ItemKind;
  items: PaletteItem[];
}

const GROUPS: Record<Mode, ItemKind[]> = {
  objects: ['table', 'view', 'function', 'database', 'session', 'user'],
  library: ['script', 'query', 'connection', 'space', 'note', 'snippet'],
  commands: ['command'],
};

const LIMIT = 80;
const PER_GROUP = 40;

function mayHaveKey(keys: Iterable<string>): (it: PaletteItem) => boolean {
  const names = new Set<string>();
  for (const k of keys) for (const part of k.split(SEP)) names.add(part.split('(')[0] ?? part);
  if (names.size === 0) return () => false;
  return (it) => !(it instanceof SchemaItem) || names.has(it.label);
}

const byMode = new WeakMap<readonly PaletteItem[], Map<Mode, PaletteItem[]>>();

function ofMode(all: readonly PaletteItem[], mode: Mode): PaletteItem[] {
  let modes = byMode.get(all);
  if (!modes) byMode.set(all, (modes = new Map()));
  let mine = modes.get(mode);
  if (!mine) modes.set(mode, (mine = all.filter((it) => MODE_OF[it.kind] === mode)));
  return mine;
}

export function sectionsOf(all: readonly PaletteItem[], mode: Mode, query: string, marks: readonly Mark[], recent: readonly string[]): Section[] {
  return drain(sectionSteps(all, mode, query, marks, recent));
}

export function* sectionSteps(all: readonly PaletteItem[], mode: Mode, query: string, marks: readonly Mark[], recent: readonly string[]): Generator<void, Section[]> {
  const mine = ofMode(all, mode);
  const byMark = new Map(marks.map((mk) => [markKey(mk), mk.pinned ? 400 : 240]));
  if (query.trim()) {
    const recentAt = new Map(recent.map((k, i) => [k, 160 - i * 3]));
    const keyed = mayHaveKey([...recentAt.keys(), ...byMark.keys()]);
    const boost = (it: PaletteItem) => (keyed(it) ? KIND_BOOST[it.kind] + (recentAt.get(it.key) ?? 0) + (byMark.get(it.key) ?? 0) : KIND_BOOST[it.kind]);
    const ranked = yield* rankSteps(mine, query, boost, LIMIT);
    return [{ id: 'results', items: ranked.map((r) => r.item) }];
  }
  const pinnedKeys = marks.filter((mk) => mk.pinned).map(markKey);
  const bookmarkKeys = marks.filter((mk) => !mk.pinned && mk.target.kind !== 'item').map(markKey);
  const wanted = new Set([...pinnedKeys, ...bookmarkKeys, ...recent]);
  const found = new Map<string, PaletteItem>();
  const limit = mode === 'commands' ? Infinity : PER_GROUP;
  const groups = new Map<ItemKind, PaletteItem[]>(GROUPS[mode].map((k) => [k, []]));
  const keyed = mayHaveKey(wanted);
  for (const it of mine) {
    if (keyed(it) && wanted.has(it.key)) found.set(it.key, it);
    const g = groups.get(it.kind);
    if (g && g.length < limit + wanted.size) g.push(it);
  }
  const taken = new Set<string>();
  const pick = (keys: readonly string[], cap = Infinity) => {
    const out: PaletteItem[] = [];
    for (const k of keys) {
      const it = found.get(k);
      if (!it || taken.has(k) || out.length >= cap) continue;
      taken.add(k);
      out.push(it);
    }
    return out;
  };
  const sections: Section[] = [
    { id: 'pinned', items: pick(pinnedKeys) },
    { id: 'bookmarks', items: pick(bookmarkKeys) },
    { id: 'recent', items: pick(recent, 8) },
  ];
  for (const [kind, rows] of groups) sections.push({ id: kind, items: rows.filter((it) => !taken.has(it.key)).slice(0, limit) });
  return sections.filter((s) => s.items.length > 0);
}
