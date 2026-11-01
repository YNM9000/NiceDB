import { hasTool, sqlOf } from './family';
import { BookmarkMinus, PinOff } from '@lucide/svelte';
import { commandIcon, ICON, TOOL_ICON, type Icon } from './icons';
import { untrack } from 'svelte';
import { m } from '../paraglide/messages';
import { api, type ConnectionProfile, type Mark, type MarkTarget, type Schema } from './api';
import { connections } from './connections.svelte';
import { copyText } from './contextmenu.svelte';
import { copyDdl, showDdl } from './ddl.svelte';
import { sqlgenActions, subjectIn } from './sqlgen.svelte';
import { openDesigner } from './design.svelte';
import type { ObjectRef } from './ddl';
import { serverOf } from './explorer-state.svelte';
import { loadServer, openRelation, queryOn, showRelationEr, showTool, showUser, tabOn } from './explorer.svelte';
import { library, openScript } from './library.svelte';
import { spaceSize, spaces, switchSpace } from './org.svelte';
import { panel, togglePanel } from './panel.svelte';
import { currentBindings } from './prefs.svelte';
import { matches } from './shortcuts';
import { inSlices, prepare, rank } from './fuzzy';
import { buildSchemaItems, item, markItem, markKey, noteTitle, queryLabel, schemaItems, sectionSteps, sectionsOf, serverItems, touch, type ItemKind, type Mode, type PaletteItem, type Section, type Words } from './palette-items';
import { snippetsFor, plain } from './snippets';
import { limitedSql, qualifiedName, quoteName, selectSql } from './sqlname';
import { showToast } from './toast.svelte';
import { askExport, askImport, killServerSession } from './tools.svelte';
import { variantOfSession, type ToolKind } from './tools';
import { MAINTENANCE_LABEL, maintenanceOf } from './monitor';
import { maintain } from './monitor.svelte';
import { seedGenerate } from './generate';
import { record } from './undo.svelte';
import { activeTab, ensureSession, newTab, openConnection, sessionFor, workspace } from './workspace.svelte';

export interface Command {
  id: string;
  label: () => string;
  group: () => string;
  keys?: () => string | null;
  when?: () => boolean;
  run: () => unknown;
  icon?: Icon;
  ask?: Ask;
}

export interface Choice {
  id: string;
  label: string;
  where?: string;
  icon?: Icon;
}

export interface Ask {
  prompt: () => string;
  choices?: () => Choice[];
  run: (value: string) => unknown;
}

export interface PaletteAction {
  id: string;
  label: string;
  icon: Icon;
  danger?: boolean;
  run: () => unknown;
}

interface EditorLike {
  insertSnippet: (template: string) => void;
}

export const palette = $state({ open: false, mode: 'objects' as Mode, query: '', marks: [] as Mark[], recent: [] as string[], loaded: false, version: 0, asking: null as string | null });

export const paletteHost: { editor: EditorLike | null; editConnection: ((p: ConnectionProfile) => void) | null; connectionDialog: ((p: ConnectionProfile, mode: 'edit' | 'copy' | 'delete') => void) | null } = { editor: null, editConnection: null, connectionDialog: null };

const registry = new Map<string, Command>();

const bump = () => untrack(() => palette.version++);

export function registerCommands(list: readonly Command[]): () => void {
  for (const c of list) registry.set(c.id, c);
  bump();
  return () => {
    for (const c of list) if (registry.get(c.id) === c) registry.delete(c.id);
    bump();
  };
}

const ORDER = ['query', 'editor', 'panel', 'view', 'tab', 'schema', 'connection', 'tool', 'palette', 'app'];
const orderOf = (c: Command) => {
  const at = ORDER.indexOf(c.id.split('.')[0] ?? '');
  return at < 0 ? ORDER.length : at;
};

export function commandList(): Command[] {
  void palette.version;
  return [...registry.values()].filter((c) => c.when?.() ?? true).sort((a, b) => orderOf(a) - orderOf(b));
}

export async function loadMarks() {
  if (palette.loaded) return;
  const saved = await api.marksGet().catch(() => null);
  palette.marks = saved?.marks ?? [];
  palette.recent = saved?.recent ?? [];
  palette.loaded = true;
}

async function saveMarks() {
  const saved = await api.marksSet({ marks: $state.snapshot(palette.marks) as Mark[], recent: [...palette.recent] });
  palette.marks = saved.marks;
}

export function openPalette(mode: Mode = 'objects', query = '') {
  palette.mode = mode;
  palette.query = query;
  palette.asking = null;
  palette.open = true;
  void loadMarks();
  refreshLive();
}

export const LIVE_EVERY = 5000;
const liveAt = new Map<string, number>();

export function refreshLive(now = Date.now()) {
  for (const cid of Object.keys(workspace.sessions)) {
    if ((liveAt.get(cid) ?? 0) + LIVE_EVERY > now + 50) continue;
    const srv = serverOf(cid);
    const groups = (['sessions', 'users'] as const).filter((g) => srv[g].status === 'ready');
    if (!groups.length) continue;
    liveAt.set(cid, now);
    for (const g of groups) void loadServer(cid, g, true);
  }
}

export const closePalette = () => {
  palette.open = false;
  palette.asking = null;
};

export const commandOf = (id: string | null) => (id ? (registry.get(id) ?? null) : null);

const words = (): Words => ({ table: m.pal_find_table(), view: m.pal_find_view(), column: m.pal_find_column(), function: m.pal_find_function(), sequence: m.pal_find_sequence(), type: m.pal_find_type(), trigger: m.pal_find_trigger(), index: m.pal_find_index(), unique: m.pal_index_unique(), database: m.pal_find_database(), session: m.pal_find_session(), user: m.pal_find_user() });

const nameOf = (connectionId: string | null) => (connectionId ? (connections.list.find((c) => c.id === connectionId)?.name ?? connectionId) : '');

const built = new WeakMap<Schema, { name: string; main: boolean; items: PaletteItem[] }>();

function itemsOf(connectionId: string, schema: Schema, main: boolean, w: Words): PaletteItem[] {
  const name = nameOf(connectionId);
  const hit = built.get(schema);
  if (hit && hit.name === name && hit.main === main) return hit.items;
  const items = untrack(() => schemaItems({ connectionId, connection: name, schema, main, words: w }));
  built.set(schema, { name, main, items });
  return items;
}

export function objectItems(): PaletteItem[] {
  const w = words();
  const lists: (readonly PaletteItem[])[] = [];
  for (const [cid, s] of Object.entries(workspace.sessions)) {
    if (s.schema) lists.push(itemsOf(cid, s.schema, true, w));
    for (const extra of Object.values(workspace.extra[cid] ?? {})) if (extra.schema) lists.push(itemsOf(cid, extra.schema, false, w));
    const srv = serverOf(cid);
    lists.push(serverItems(cid, nameOf(cid), srv.sessions.data, srv.users.data, w));
  }
  const out = lists.flat();
  const tables = palette.marks.filter((mk) => mk.target.kind === 'table');
  if (!tables.length) return out;
  const seen = new Set(out.map((it) => it.key));
  for (const mk of tables) {
    const t = mk.target;
    if (t.kind !== 'table' || seen.has(markKey(mk)) || !connections.list.some((c) => c.id === t.connection_id)) continue;
    const it = markItem(mk, nameOf(t.connection_id));
    if (it) out.push(it);
  }
  return out;
}

export function libraryItems(): PaletteItem[] {
  const out: PaletteItem[] = [];
  for (const s of library.scripts) {
    out.push(item({ key: `script:${s.id}`, kind: 'script', label: s.name, where: [s.folder, s.connection_id ? nameOf(s.connection_id) : m.panel_scripts_any()].filter(Boolean).join(' · '), detail: s.tags.map((t) => `#${t}`).join(' '), words: `${s.tags.join(' ')} ${s.sql.slice(0, 200)}`, connectionId: s.connection_id, ref: s.id }));
  }
  for (const mk of palette.marks) {
    const t = mk.target;
    if (t.kind !== 'query' || !connections.list.some((c) => c.id === t.connection_id)) continue;
    const it = markItem(mk, nameOf(t.connection_id));
    if (it) out.push(it);
  }
  for (const n of library.notes) {
    const where = n.scope === 'connection' ? nameOf(n.scope_id) : (spaces.list.find((w) => w.id === n.scope_id)?.name ?? m.panel_notes_all());
    out.push(item({ key: `note:${n.scope}:${n.scope_id}`, kind: 'note', label: noteTitle(n.body) || m.panel_notes(), where, words: n.body.slice(0, 400), connectionId: n.scope === 'connection' ? n.scope_id : null, ref: n.scope_id }));
  }
  const tab = activeTab();
  const dialect = tab ? sessionFor(tab.connectionId, tab.database)?.info.dialect : undefined;
  for (const s of dialect ? snippetsFor(dialect, library.snippets) : library.snippets.map((x) => ({ ...x, builtin: false }))) {
    out.push(item({ key: `snippet:${s.id}`, kind: 'snippet', label: s.name, where: s.description, detail: s.trigger, words: s.trigger, ref: s.id }));
  }
  for (const c of connections.list) {
    const target = c.target;
    const host = 'host' in target ? `${target.host} ${target.database}` : 'path' in target ? target.path : '';
    out.push(item({ key: `conn:${c.id}`, kind: 'connection', label: c.name, where: ENV[c.environment](), detail: workspace.sessions[c.id] ? m.pal_open() : '', words: `${host} ${m.pal_word_connection()}`, connectionId: c.id }));
  }
  out.push(item({ key: 'space:', kind: 'space', label: m.org_space_all(), where: m.status_connections({ count: connections.list.length }), words: m.pal_word_space(), detail: spaces.active === null ? m.pal_current() : '', ref: null }));
  for (const w of spaces.list) out.push(item({ key: `space:${w.id}`, kind: 'space', label: w.name, where: m.status_connections({ count: spaceSize(w) }), words: m.pal_word_space(), detail: spaces.active === w.id ? m.pal_current() : '', ref: w.id }));
  return out;
}

const ENV = { local: m.env_local, development: m.env_development, staging: m.env_staging, production: m.env_production } as const;

export function commandItems(): PaletteItem[] {
  return commandList().map((c) => item({ key: `cmd:${c.id}`, kind: 'command', label: c.label(), where: c.group(), detail: c.keys?.() ?? '', ref: c.id }));
}

const markIndex = $derived(new Map(palette.marks.map((mk) => [markKey(mk), mk])));
export const markOf = (key: string) => markIndex.get(key) ?? null;
export const isPinned = (key: string) => !!markOf(key)?.pinned;
export const isBookmarked = (key: string) => {
  const mk = markOf(key);
  return !!mk && mk.target.kind !== 'item';
};

export const search = (all: readonly PaletteItem[], mode: Mode, query: string): Section[] => sectionsOf(all, mode, query, $state.snapshot(palette.marks) as Mark[], $state.snapshot(palette.recent));

export const SLICED_FROM = 50_000;

export const searchSliced = (all: readonly PaletteItem[], mode: Mode, query: string, done: (sections: Section[]) => void) =>
  inSlices(sectionSteps(all, mode, query, $state.snapshot(palette.marks) as Mark[], $state.snapshot(palette.recent)), done);

async function setMarks(next: Mark[], label: string) {
  const before = $state.snapshot(palette.marks) as Mark[];
  palette.marks = next;
  await saveMarks();
  record({
    label,
    undo: async () => {
      palette.marks = before;
      await saveMarks();
    },
    redo: async () => {
      palette.marks = next;
      await saveMarks();
    },
  });
}

function targetOf(it: PaletteItem): MarkTarget {
  if ((it.kind === 'table' || it.kind === 'view') && it.connectionId && it.database && it.namespace !== null) return { kind: 'table', connection_id: it.connectionId, database: it.database, namespace: it.namespace, name: it.label, view: it.kind === 'view' };
  return { kind: 'item', key: it.key };
}

export async function togglePin(it: PaletteItem) {
  const mk = markOf(it.key);
  const rest = palette.marks.filter((x) => x !== mk);
  if (mk?.pinned) {
    await setMarks(mk.target.kind === 'item' ? rest : palette.marks.map((x) => (x === mk ? { ...x, pinned: false } : x)), m.pal_undo_unpinned({ name: it.label }));
    return;
  }
  const next: Mark = mk ? { ...mk, pinned: true } : { id: crypto.randomUUID(), label: it.label, pinned: true, target: targetOf(it) };
  await setMarks([next, ...rest], m.pal_undo_pinned({ name: it.label }));
}

export async function toggleBookmark(it: PaletteItem) {
  const mk = markOf(it.key);
  if (mk && mk.target.kind !== 'item') {
    await setMarks(palette.marks.filter((x) => x !== mk), m.pal_undo_unbookmarked({ name: it.label }));
    return;
  }
  const target = targetOf(it);
  if (target.kind === 'item') return;
  await setMarks([...palette.marks.filter((x) => x !== mk), { id: crypto.randomUUID(), label: it.label, pinned: !!mk?.pinned, target }], m.pal_undo_bookmarked({ name: it.label }));
}

export async function bookmarkQuery(connectionId: string, database: string | null, sql: string) {
  const text = sql.trim();
  if (!text) return;
  const label = queryLabel(text);
  await setMarks([...palette.marks, { id: crypto.randomUUID(), label, pinned: false, target: { kind: 'query', connection_id: connectionId, database, sql: text } }], m.pal_undo_bookmarked({ name: label }));
  showToast(m.pal_query_bookmarked({ name: label }));
}

async function copied(text: string, what: string) {
  await copyText(text);
  showToast(m.pal_copied({ what }));
}

function objectRef(it: PaletteItem): ObjectRef | null {
  const ns = it.namespace ?? '';
  if (it.kind === 'table') return { kind: 'table', namespace: ns, name: it.label, table: null, signature: null };
  if (it.kind === 'view') return { kind: 'view', namespace: ns, name: it.label, table: null, signature: null };
  if (it.kind === 'function') return { kind: 'routine', namespace: ns, name: it.label, table: null, signature: it.ref };
  if (it.kind === 'sequence') return { kind: 'sequence', namespace: ns, name: it.label, table: null, signature: null };
  if (it.kind === 'trigger') return { kind: 'trigger', namespace: ns, name: it.label, table: it.ref, signature: null };
  if (it.kind === 'index') return { kind: 'index', namespace: ns, name: it.label, table: it.ref, signature: null };
  return null;
}

async function ddlOf(it: PaletteItem, object: ObjectRef) {
  const cid = it.connectionId;
  const database = it.database;
  if (!cid || !database || !(await ensureSession(cid, database))) return;
  const s = sessionFor(cid, database);
  if (s) showDdl({ connectionId: cid, database, dialect: sqlOf(s.info), product: s.info.server.product, object });
}

const dialectOf = (it: PaletteItem) => (it.connectionId ? sessionFor(it.connectionId, it.database)?.info.dialect : undefined) ?? 'postgres';

function relationActions(it: PaletteItem): PaletteAction[] {
  const cid = it.connectionId ?? '';
  const db = it.database ?? '';
  const ns = it.namespace ?? '';
  const object = objectRef(it);
  const d = dialectOf(it);
  const full = qualifiedName(d, ns, it.label, db);
  const table = it.kind === 'table';
  const info = sessionFor(cid, db)?.info ?? null;
  return [
    { id: 'data', label: m.tree_open_data(), icon: ICON.openData, run: () => openRelation(cid, db, ns, it.label) },
    { id: 'query', label: m.tree_new_query(), icon: ICON.newQuery, run: () => queryOn(cid, db, selectSql(d, ns, it.label, db)) },
    ...(object ? [{ id: 'ddl', label: m.ddl_show(), icon: ICON.ddl, run: () => ddlOf(it, object) }] : []),
    ...(it.kind === 'table' ? [{ id: 'design', label: m.dz_edit_table_dots(), icon: ICON.design, run: () => void openDesigner(cid, db, ns, it.label) }] : []),
    { id: 'er', label: m.tree_show_er(), icon: ICON.er, run: () => showRelationEr(cid, db, ns, it.label) },
    { id: 'copy', label: m.tree_copy_name(), icon: ICON.copy, run: () => copied(full, full) },
    { id: 'select', label: m.tree_copy_select(), icon: ICON.copy, run: () => copied(selectSql(d, ns, it.label, db), it.label) },
    ...(object ? [{ id: 'create', label: m.tree_copy_create(), icon: ICON.copy, run: () => copyDdl(cid, db, object) }] : []),
    { id: 'export', label: m.tool_export_data_dots(), icon: ICON.export, run: () => askExport({ connectionId: cid, database: db, source: { kind: 'table', namespace: ns, name: it.label }, label: it.label, expected: it.count }) },
    ...(table
      ? [
          { id: 'import', label: m.tool_import_data_dots(), icon: ICON.import, run: () => askImport({ connectionId: cid, database: db, table: { namespace: ns, name: it.label } }) },
          { id: 'generate', label: m.tool_generate_dots(), icon: TOOL_ICON.generate, run: () => (seedGenerate(ns, it.label), showTool(cid, db, 'generate')) },
        ]
      : []),
    ...sqlgenActions(cid, subjectIn(cid, d, db, ns, it.label, it.kind === 'view')),
    ...(table && info ? maintenanceOf(variantOfSession(info), 'table').map((action) => ({ id: `maintain-${action}`, label: MAINTENANCE_LABEL[action](), icon: ICON.maintain, run: () => maintain(cid, db, action, { namespace: ns, name: it.label }) })) : []),
  ];
}

function kindActions(it: PaletteItem): PaletteAction[] {
  const cid = it.connectionId ?? '';
  const db = it.database ?? '';
  switch (it.kind) {
    case 'table':
    case 'view':
      return relationActions(it);
    case 'column': {
      const d = dialectOf(it);
      const table = it.ref ?? '';
      const ns = it.namespace ?? '';
      const owner = qualifiedName(d, ns, table, db);
      const col = quoteName(d, it.label);
      return [
        { id: 'data', label: m.pal_open_table({ table }), icon: ICON.openData, run: () => openRelation(cid, db, ns, table) },
        { id: 'copy', label: m.tree_copy_name(), icon: ICON.copy, run: () => copied(col, col) },
        { id: 'qualified', label: m.tree_copy_qualified(), icon: ICON.copy, run: () => copied(`${owner}.${col}`, `${owner}.${col}`) },
        { id: 'query', label: m.pal_query_column(), icon: ICON.newQuery, run: () => queryOn(cid, db, limitedSql(d, `${col}, count(*)`, `FROM ${owner} GROUP BY ${col} ORDER BY 2 DESC`), false) },
      ];
    }
    case 'function': {
      const object = objectRef(it);
      const sig = `${qualifiedName(dialectOf(it), it.namespace ?? '', it.label, db)}${it.detail}`;
      return [
        ...(object ? [{ id: 'ddl', label: m.ddl_show(), icon: ICON.ddl, run: () => ddlOf(it, object) }] : []),
        { id: 'signature', label: m.tree_copy_signature(), icon: ICON.copy, run: () => copied(sig, sig) },
        { id: 'copy', label: m.tree_copy_name(), icon: ICON.copy, run: () => copied(it.label, it.label) },
      ];
    }
    case 'sequence':
    case 'type':
    case 'trigger':
    case 'index': {
      const object = objectRef(it);
      const ns = it.namespace ?? '';
      const full = qualifiedName(dialectOf(it), ns, it.label, db);
      const owner = it.kind === 'trigger' || it.kind === 'index' ? it.ref : null;
      return [
        ...(object && sessionFor(cid, db)?.info.caps.ddl !== false ? [{ id: 'ddl', label: m.ddl_show(), icon: ICON.ddl, run: () => ddlOf(it, object) }] : []),
        ...(owner ? [{ id: 'data', label: m.pal_open_table({ table: owner }), icon: ICON.openData, run: () => openRelation(cid, db, ns, owner) }] : []),
        { id: 'copy', label: m.tree_copy_name(), icon: ICON.copy, run: () => copied(it.label, it.label) },
        { id: 'qualified', label: m.tree_copy_qualified(), icon: ICON.copy, run: () => copied(full, full) },
        ...(object ? [{ id: 'create', label: m.tree_copy_create(), icon: ICON.copy, run: () => copyDdl(cid, db, object) }] : []),
      ];
    }
    case 'database':
      return [
        { id: 'open', label: m.ex_open_db(), icon: ICON.openDatabase, run: () => tabOn(cid, db) },
        { id: 'query', label: m.ex_query_here(), icon: ICON.newQuery, run: () => queryOn(cid, db, '') },
        ...databaseTools(cid, db),
        { id: 'copy', label: m.tree_copy_name(), icon: ICON.copy, run: () => copied(db, db) },
      ];
    case 'session':
      return sessionActions(it);
    case 'user':
      return [
        { id: 'grants', label: m.ex_permissions(), icon: ICON.permissions, run: () => showUser(cid, it.label) },
        { id: 'copy', label: m.tree_copy_name(), icon: ICON.copy, run: () => copied(it.label, it.label) },
      ];
    case 'script':
      return scriptActions(it);
    case 'query':
      return queryActions(it);
    case 'note':
      return [{ id: 'open', label: m.pal_open_note(), icon: ICON.notes, run: () => openNote(it) }];
    case 'snippet': {
      const s = [...library.snippets, ...snippetsFor(dialectOf(it), [])].find((x) => x.id === it.ref);
      const editor = paletteHost.editor;
      return [
        ...(editor && s ? [{ id: 'insert', label: m.history_insert(), icon: ICON.newQuery, run: () => editor.insertSnippet(s.body) }] : []),
        ...(s ? [{ id: 'copy', label: m.pal_copy_sql(), icon: ICON.copy, run: () => copied(plain(s.body), s.name) }] : []),
      ];
    }
    case 'connection': {
      const profile = connections.list.find((c) => c.id === cid);
      const open = !!workspace.sessions[cid];
      if (profile?.template.kind === 'model' && paletteHost.editConnection) {
        return [
          { id: 'edit', label: m.conn_edit_dots(), icon: ICON.edit, run: () => paletteHost.editConnection?.(profile) },
          { id: 'copy', label: m.tree_copy_name(), icon: ICON.copy, run: () => copied(it.label, it.label) },
        ];
      }
      return [
        { id: 'connect', label: open ? m.pal_go_to() : m.pal_connect(), icon: ICON.connect, run: () => openConnection(cid) },
        { id: 'query', label: m.tab_new(), icon: ICON.newQuery, run: async () => (await ensureSession(cid, null)) && newTab(cid) },
        ...(profile && paletteHost.editConnection ? [{ id: 'edit', label: m.conn_edit_dots(), icon: ICON.edit, run: () => paletteHost.editConnection?.(profile) }] : []),
        { id: 'copy', label: m.tree_copy_name(), icon: ICON.copy, run: () => copied(it.label, it.label) },
      ];
    }
    case 'space':
      return [{ id: 'switch', label: m.pal_switch_space(), icon: ICON.space, run: () => switchSpace(it.ref) }];
    case 'command': {
      const c = registry.get(it.ref ?? '');
      return c ? [{ id: 'run', label: c.ask ? m.pal_run_ask() : m.pal_run_command(), icon: commandIcon(c.id, c.icon), run: c.ask ? () => (palette.asking = c.id) : c.run }] : [];
    }
  }
}

const DATABASE_TOOLS: [ToolKind, () => string][] = [
  ['search', m.tool_search_dots],
  ['compare', m.tool_compare],
  ['backup', m.tool_backup],
  ['seclab', m.tool_seclab],
  ['space', m.tool_space],
  ['monitor', m.tool_monitor],
];

function databaseTools(cid: string, db: string): PaletteAction[] {
  const caps = sessionFor(cid)?.info.caps;
  if (!caps) return [];
  return DATABASE_TOOLS.filter(([tool]) => hasTool(caps, tool)).map(([tool, label]) => ({ id: `tool-${tool}`, label: label(), icon: TOOL_ICON[tool], run: () => showTool(cid, db, tool) }));
}

function sessionActions(it: PaletteItem): PaletteAction[] {
  const cid = it.connectionId ?? '';
  const id = it.ref ?? '';
  const s = serverOf(cid).sessions.data?.sessions.find((x) => x.id === id);
  const kill = async (mode: 'cancel' | 'terminate') => {
    const main = sessionFor(cid);
    if (!main) return;
    const said = await killServerSession(main.info, { id, query: s?.query ?? null }, mode);
    if (said === null) return;
    showToast(said);
    await loadServer(cid, 'sessions');
  };
  return [
    { id: 'cancel', label: m.tool_kill_cancel(), icon: ICON.kill, run: () => kill('cancel') },
    ...(s?.query ? [{ id: 'copy', label: m.tool_copy_query(), icon: ICON.copy, run: () => copied(s.query ?? '', id) }] : []),
    { id: 'activity', label: m.ex_open_activity(), icon: TOOL_ICON.activity, run: () => showTool(cid, null, 'activity') },
    { id: 'terminate', label: m.tool_kill_close(), icon: ICON.terminate, danger: true, run: () => kill('terminate') },
  ];
}

function scriptActions(it: PaletteItem): PaletteAction[] {
  const s = library.scripts.find((x) => x.id === it.ref);
  if (!s) return [];
  const fallback = activeTab()?.connectionId ?? Object.keys(workspace.sessions)[0] ?? null;
  const usable = !!(s.connection_id ?? fallback);
  return [
    ...(usable ? [{ id: 'open', label: m.pal_open_script(), icon: ICON.scripts, run: () => openScript(s, fallback) }, { id: 'run', label: m.panel_scripts_run(), icon: ICON.run, run: () => openScript(s, fallback, true) }] : []),
    { id: 'copy', label: m.pal_copy_sql(), icon: ICON.copy, run: () => copied(s.sql, s.name) },
  ];
}

function queryActions(it: PaletteItem): PaletteAction[] {
  const mk = palette.marks.find((x) => x.id === it.ref);
  if (mk?.target.kind !== 'query') return [];
  const { connection_id: cid, database, sql } = mk.target;
  const open = async (run: boolean) => {
    if (await ensureSession(cid, null)) await queryOn(cid, database ?? sessionFor(cid)?.database ?? '', sql, run);
  };
  return [
    { id: 'open', label: m.tool_open_editor(), icon: ICON.scripts, run: () => open(false) },
    { id: 'run', label: m.panel_scripts_run(), icon: ICON.run, run: () => open(true) },
    { id: 'copy', label: m.pal_copy_sql(), icon: ICON.copy, run: () => copied(sql, mk.label) },
  ];
}

async function openNote(it: PaletteItem) {
  const cid = it.connectionId;
  if (cid) await openConnection(cid);
  else if (it.ref && it.ref !== spaces.active) await switchSpace(it.ref);
  if (workspace.tabs.length && !(panel.open && panel.tab === 'notes')) togglePanel('notes');
}

export function actionsOf(it: PaletteItem): PaletteAction[] {
  const own = kindActions(it);
  const pinned = isPinned(it.key);
  const marks: PaletteAction[] = [{ id: 'pin', label: pinned ? m.pal_unpin() : m.pal_pin(), icon: pinned ? PinOff : ICON.pin, run: () => togglePin(it) }];
  if (it.kind === 'table' || it.kind === 'view') marks.push({ id: 'bookmark', label: isBookmarked(it.key) ? m.pal_unbookmark() : m.pal_bookmark(), icon: isBookmarked(it.key) ? BookmarkMinus : ICON.bookmark, run: () => toggleBookmark(it) });
  if (it.kind === 'query') marks.push({ id: 'bookmark', label: m.pal_unbookmark(), icon: BookmarkMinus, run: () => toggleBookmark(it) });
  return [...own, ...marks];
}

export async function choose(it: PaletteItem, action: PaletteAction | undefined) {
  if (!action) return;
  const marking = action.id === 'pin' || action.id === 'bookmark';
  const asks = it.kind === 'command' && !!registry.get(it.ref ?? '')?.ask;
  if (asks) {
    palette.recent = touch(palette.recent, it.key);
    void saveMarks().catch(() => {});
    palette.query = '';
  } else if (!marking) {
    palette.recent = touch(palette.recent, it.key);
    closePalette();
    void saveMarks().catch(() => {});
  }
  try {
    await action.run();
  } catch (e) {
    showToast(e instanceof Error ? e.message : String(e), null, 'error');
  }
}

export const kindLabel = (kind: ItemKind): string =>
  ({
    table: m.pal_word_table,
    view: m.pal_word_view,
    column: m.pal_word_column,
    function: m.pal_word_function,
    sequence: m.pal_word_sequence,
    type: m.pal_word_type,
    trigger: m.pal_word_trigger,
    index: m.pal_word_index,
    database: m.pal_word_database,
    session: m.pal_word_session,
    user: m.pal_word_user,
    script: m.pal_word_script,
    note: m.pal_word_note,
    snippet: m.pal_word_snippet,
    connection: m.pal_word_connection,
    space: m.pal_word_space,
    query: m.pal_word_query,
    command: m.pal_word_command,
  })[kind]();

export function paletteKeys(allowed: () => boolean, target: Window = window) {
  const onkey = (e: KeyboardEvent) => {
    const b = currentBindings();
    const mode: Mode | null = matches(e, b.palette) ? 'objects' : matches(e, b.palette_commands) ? 'commands' : null;
    if (!mode || !allowed() || document.querySelector('dialog[open]:not(.cmd-palette)')) return;
    e.preventDefault();
    if (palette.open && palette.mode === mode) closePalette();
    else if (palette.open) palette.mode = mode;
    else openPalette(mode);
  };
  target.addEventListener('keydown', onkey);
  return () => target.removeEventListener('keydown', onkey);
}

export function warmPalette() {
  const pending: [string, Schema, boolean][] = [];
  for (const [cid, s] of Object.entries(workspace.sessions)) {
    if (s.schema) pending.push([cid, s.schema, true]);
    for (const extra of Object.values(workspace.extra[cid] ?? {})) if (extra.schema) pending.push([cid, extra.schema, false]);
  }
  const todo = pending.filter(([, schema]) => !built.has(schema));
  if (!todo.length) return;
  let timer = setTimeout(() => {
    const w = words();
    const jobs = untrack(() => todo.map(([cid, schema, main]) => ({ cid, schema, main, name: nameOf(cid), build: buildSchemaItems({ connectionId: cid, connection: nameOf(cid), schema, main, words: w }) })));
    const slice = () => {
      const until = performance.now() + WARM_SLICE_MS;
      while (jobs.length) {
        const job = jobs[0];
        if (!job) break;
        if (built.has(job.schema)) {
          jobs.shift();
          continue;
        }
        const step = job.build.next();
        if (step.done) {
          built.set(job.schema, { name: job.name, main: job.main, items: step.value });
          jobs.shift();
        }
        if (performance.now() > until) {
          timer = setTimeout(slice, 0);
          return;
        }
      }
    };
    slice();
  }, 400);
  return () => clearTimeout(timer);
}

const WARM_SLICE_MS = 8;

export function rankChoices(choices: readonly Choice[], query: string): Choice[] {
  if (!query.trim()) return [...choices];
  const items = choices.map((c) => ({ c, name: prepare(c.label), context: prepare(c.where ?? '') }));
  return rank(items, query, () => 0, 200).map((r) => r.item.c);
}

export async function answer(value: string) {
  const c = commandOf(palette.asking);
  closePalette();
  if (!c?.ask || !value) return;
  try {
    await c.ask.run(value);
  } catch (e) {
    showToast(e instanceof Error ? e.message : String(e), null, 'error');
  }
}

export function stopAsking() {
  palette.asking = null;
  palette.query = '';
  palette.mode = 'commands';
}
