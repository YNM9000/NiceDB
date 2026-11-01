import { m } from '../paraglide/messages';
import { api, ApiError, type Schema } from './api';
import { dbs, saveVisibility, visibilityOf } from './dbs.svelte';
import { keyOf, type Loaded } from './explorer';
import { explorer, serverOf, usersFocus, want, writableServer } from './explorer-state.svelte';
import { record } from './undo.svelte';
import { setDatabase, setSchema } from './visibility';
import { openTool } from './tools.svelte';
import { activeTab, closeTab, connect, databaseOf, ensureSession, newTab, openTable, refreshSchema, releaseDatabase, run, sessionFor, showInEr, useDatabase, workspace, type Tab } from './workspace.svelte';
import type { ToolKind } from './tools';
import { browseText, viewsOf } from './family';

const sessionKey = (connectionId: string, database: string) => `${connectionId}\u0000${database}`;

export const toggleKey = (key: string) => (explorer.open[key] = !explorer.open[key]);

export function loadedSchema(connectionId: string, database: string): Loaded<Schema> {
  const s = sessionFor(connectionId, database);
  if (s?.schema) return { status: 'ready', data: s.schema, error: null };
  const failure = s?.schemaError ?? explorer.failures[keyOf(connectionId, 'db', database)] ?? null;
  if (failure) return { status: 'failed', data: null, error: failure };
  return { status: s || workspace.opening[sessionKey(connectionId, database)] ? 'loading' : 'idle', data: null, error: null };
}

const pending = new Set<string>();

export async function expandDatabase(connectionId: string, database: string) {
  const key = keyOf(connectionId, 'db', database);
  if (pending.has(key)) return;
  pending.add(key);
  try {
    delete explorer.failures[key];
    explorer.open[keyOf(key, 'schemas')] ??= true;
    want(connectionId, database, true);
    if (await ensureSession(connectionId, database)) return;
    explorer.failures[key] = workspace.openErrors[sessionKey(connectionId, database)]?.message ?? m.dbs_open_failed();
    delete workspace.openErrors[sessionKey(connectionId, database)];
    want(connectionId, database, false);
  } finally {
    pending.delete(key);
  }
}

export async function collapseDatabase(connectionId: string, database: string) {
  want(connectionId, database, false);
  await releaseDatabase(connectionId);
}

export async function toggleDatabase(connectionId: string, database: string) {
  const key = keyOf(connectionId, 'db', database);
  if (explorer.open[key]) {
    explorer.open[key] = false;
    await collapseDatabase(connectionId, database);
    return;
  }
  explorer.open[key] = true;
  await expandDatabase(connectionId, database);
}

export async function reloadDatabase(connectionId: string, database: string) {
  await refreshSchema(connectionId, database);
}

const TOOL_LOAD = {
  users: async (id: string, sid: string, quiet: boolean) => {
    const state = writableServer(id);
    if (!quiet) state.users = { status: 'loading', data: null, error: null };
    try {
      state.users = { status: 'ready', data: await api.principalsList(sid), error: null };
    } catch (e) {
      if (!quiet) state.users = { status: 'failed', data: null, error: unsupportedText(e) };
    }
  },
  sessions: async (id: string, sid: string, quiet: boolean) => {
    const state = writableServer(id);
    if (!quiet) state.sessions = { status: 'loading', data: null, error: null };
    try {
      state.sessions = { status: 'ready', data: await api.serverActivity(sid), error: null };
    } catch (e) {
      if (!quiet) state.sessions = { status: 'failed', data: null, error: unsupportedText(e) };
    }
  },
};

function unsupportedText(e: unknown): string {
  if (e instanceof ApiError && e.error.code === 'tool' && e.error.detail.kind === 'unsupported') return m.tool_fault_unsupported();
  return e instanceof Error ? e.message : String(e);
}

export async function loadServer(connectionId: string, group: 'users' | 'sessions', quiet = false) {
  const s = sessionFor(connectionId);
  if (!s) return;
  const state = serverOf(connectionId);
  if ((group === 'users' ? state.users : state.sessions).status === 'loading') return;
  await TOOL_LOAD[group](connectionId, s.info.session_id, quiet);
}


export function showUser(connectionId: string, key: string) {
  usersFocus.connectionId = connectionId;
  usersFocus.key = key;
  return showTool(connectionId, null, 'users');
}

export async function tabOn(connectionId: string, database: string): Promise<Tab | null> {
  if (!(await connect(connectionId))) return null;
  const mine = workspace.tabs.filter((t) => t.connectionId === connectionId && databaseOf(t) === database);
  const active = activeTab();
  const found = (active && mine.includes(active) ? active : mine[0]) ?? null;
  if (found) {
    workspace.active = found.id;
    return found;
  }
  const tab = newTab(connectionId);
  const failed = await useDatabase(tab, database);
  if (failed !== null) {
    explorer.failures[keyOf(connectionId, 'db', database)] = failed || m.dbs_open_failed();
    await closeTab(tab.id);
    return null;
  }
  return tab;
}

export async function showTool(connectionId: string, database: string | null, tool: ToolKind) {
  if (!database) return openTool(connectionId, tool);
  const tab = await tabOn(connectionId, database);
  if (!tab) return;
  tab.view = 'tools';
  tab.tool = tool;
}

export async function queryOn(connectionId: string, database: string, sql: string, execute = true) {
  if (!(await connect(connectionId))) return;
  const tab = newTab(connectionId, sql);
  if (databaseOf(tab) !== database) {
    const failed = await useDatabase(tab, database);
    if (failed !== null) {
      explorer.failures[keyOf(connectionId, 'db', database)] = failed || m.dbs_open_failed();
      await closeTab(tab.id);
      return;
    }
  }
  if (sql && execute) void run(tab, { kind: 'script' });
}

export async function showRelationEr(connectionId: string, database: string, namespace: string, name: string) {
  const tab = await tabOn(connectionId, database);
  if (tab) showInEr(tab, namespace, name);
}

export async function openRelation(connectionId: string, database: string, namespace: string, name: string) {
  const caps = sessionFor(connectionId)?.info.caps;
  if (caps && !viewsOf(caps).includes('data')) {
    const text = browseText(caps, name, () => '');
    const open = workspace.tabs.find((t) => t.connectionId === connectionId && databaseOf(t) === database && t.text === text);
    if (!open) return queryOn(connectionId, database, text);
    workspace.active = open.id;
    open.view = 'query';
    return;
  }
  const tab = await tabOn(connectionId, database);
  if (tab) openTable(tab, namespace, name);
}

export function hideDatabase(connectionId: string, database: string, shown: boolean) {
  const before = $state.snapshot(visibilityOf(connectionId));
  const after = setDatabase(before, database, shown);
  void saveVisibility(connectionId, after);
  record({
    label: shown ? m.undo_db_shown({ name: database }) : m.undo_db_hidden({ name: database }),
    undo: () => saveVisibility(connectionId, before),
    redo: () => saveVisibility(connectionId, after),
    valid: () => connectionId in workspace.sessions,
  });
}

export function hideSchema(connectionId: string, database: string, namespace: string, shown: boolean) {
  const before = $state.snapshot(visibilityOf(connectionId));
  const after = setSchema(before, database, namespace, shown);
  void saveVisibility(connectionId, after);
  record({
    label: shown ? m.undo_schema_shown({ name: namespace }) : m.undo_schema_hidden({ name: namespace }),
    undo: () => saveVisibility(connectionId, before),
    redo: () => saveVisibility(connectionId, after),
    valid: () => connectionId in workspace.sessions,
  });
}

export const showingAll = (connectionId: string) => !!dbs.showAll[connectionId];

export function seedOpen(connectionId: string, database: string, schema: Schema, flat: boolean) {
  const seen = keyOf(connectionId, 'seen', database);
  if (explorer.seeded[seen]) return;
  explorer.seeded[seen] = true;
  explorer.open[keyOf(connectionId, 'conn')] ??= true;
  const dbKey = keyOf(connectionId, 'db', database);
  const first = schema.namespaces.find((n) => n.tables.length + n.views.length > 0) ?? schema.namespaces[0];
  explorer.open[keyOf(connectionId, 'dbs')] ??= true;
  explorer.open[dbKey] ??= true;
  if (!first) return;
  if (!flat || schema.namespaces.length > 1) {
    explorer.open[keyOf(dbKey, 'schemas')] ??= true;
    explorer.open[keyOf(dbKey, 'ns', first.name)] ??= true;
  }
  explorer.open[keyOf(dbKey, 'ns', first.name, 'g', first.tables.length ? 'tables' : 'views')] ??= true;
}
