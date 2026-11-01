import { sentinelMaster } from './redis';
import { bsonType, cellText } from './ejson';
import { sqlOf } from './family';
import { m } from '../paraglide/messages';
import { api, ApiError, describe, driverMessage, paramFault, type RowPreview, type RunEvent, type Schema, type SessionEvent, type SessionInfo } from './api';
import type { Column, DriverError_Serialize, Order, Position, QueryProgress_Serialize as QueryProgress, QuickFilter, RunConfirm_Deserialize, RunScope, SortKey, Unsortable } from './bindings';
import { connections } from './connections.svelte';
import { forget, loadVisibility } from './dbs.svelte';
import { isWanted } from './explorer-state.svelte';
import { DataTab } from './data.svelte';
import { Pending } from './changeset';
import { askConfirm, refreshAccess, settle, writesOpen } from './guard.svelte';
import { play } from './sound';
import type { ToolKind } from './tools';
import { record, redoFirst, undoFirst } from './undo.svelte';
import type { Dropped, Health } from './session';
import { showToast } from './toast.svelte';
import type { Widths } from './resize';
import { forgetTx, refreshTx, setTx, settleBeforeClosing, trackSession, txGone, txOf, waiting, type CloseReason } from './tx.svelte';
import { pending } from './tx';
import { paramsOf, paramsForRun, refreshParams } from './params.svelte';
import { nameLabel } from './params';
import { prefs } from './prefs.svelte';

export type Row = (string | null)[];

export interface Done {
  elapsed: number;
  rowsAffected: number | null;
  command: string | null;
  truncated: boolean;
  rowLimit: number | null;
}

const doneOf = (e: Extract<RunEvent, { type: 'done' }>): Done => ({ elapsed: e.elapsed_ms ?? 0, rowsAffected: e.rows_affected, command: e.command, truncated: e.truncated, rowLimit: e.row_limit ?? null });

export class ResultSet {
  readonly index: number;
  readonly sql: string;
  readonly from: number;
  readonly to: number;
  rows: Row[] = [];
  columns = $state.raw<Column[]>([]);
  count = $state(0);
  done = $state.raw<Done | null>(null);
  error = $state.raw<{ error: DriverError_Serialize; position: Position | null; at: number | null; elapsed: number } | null>(null);
  readonly unsortable: Unsortable | null;
  sort = $state.raw<SortKey[]>([]);
  sorting = $state(false);
  sortError = $state<string | null>(null);
  allRows = $state(false);
  rev = $state(0);
  preview = $state.raw<RowPreview | null>(null);
  progress = $state.raw<QueryProgress | null>(null);

  constructor(index: number, sql: string, from: number, to: number, unsortable: Unsortable | null = null) {
    this.index = index;
    this.sql = sql;
    this.from = from;
    this.to = to;
    this.unsortable = unsortable;
  }

  get hasGrid() {
    return this.columns.length > 0;
  }

  docs: unknown[] = [];
  isDocs = $state(false);
  docMode = $state<'tree' | 'table'>('tree');
  private keys = new Map<string, number>();

  addDocuments(texts: readonly string[]) {
    let grew = false;
    for (const text of texts) {
      let doc: unknown;
      try {
        doc = JSON.parse(text);
      } catch {
        doc = text;
      }
      this.docs.push(doc);
      const fields = doc !== null && typeof doc === 'object' && !Array.isArray(doc) ? (doc as Record<string, unknown>) : { value: doc };
      for (const k of Object.keys(fields)) {
        if (this.keys.has(k)) continue;
        this.keys.set(k, this.keys.size);
        grew = true;
      }
      const row: (string | null)[] = new Array<string | null>(this.keys.size).fill(null);
      for (const [k, v] of Object.entries(fields)) row[this.keys.get(k) ?? 0] = cellText(v);
      this.rows.push(row);
    }
    if (grew || !this.isDocs) {
      const first = this.docs[0] as Record<string, unknown> | undefined;
      this.columns = [...this.keys.keys()].map((name) => ({ name, type_name: first && name in first ? bsonType(first[name]) : null }));
    }
    this.isDocs = true;
    this.count = this.rows.length;
  }

  replaceDocument(index: number, next: unknown | null) {
    if (next === null) {
      this.docs.splice(index, 1);
      this.rows.splice(index, 1);
    } else {
      this.docs[index] = next;
      const fields = next !== null && typeof next === 'object' ? (next as Record<string, unknown>) : {};
      const row: (string | null)[] = new Array<string | null>(this.keys.size).fill(null);
      for (const [k, v] of Object.entries(fields)) {
        const at = this.keys.get(k);
        if (at !== undefined) row[at] = cellText(v);
      }
      this.rows[index] = row;
    }
    this.count = this.rows.length;
    this.rev++;
  }
}

let tabSeq = 0;

export type View = 'query' | 'data' | 'schema' | 'tools';

export interface TableRef {
  namespace: string;
  name: string;
}

export class Tab {
  readonly id = `tab-${++tabSeq}`;
  view = $state<View>('query');
  table = $state.raw<TableRef | null>(null);
  readonly connectionId: string;
  database = $state<string | null>(null);
  title = $state('');
  text = $state('');
  results = $state.raw<ResultSet[]>([]);
  activeResult = $state(0);
  runId = $state<string | null>(null);
  finished = $state.raw<{ elapsed: number; cancelled: boolean } | null>(null);
  failure = $state<string | null>(null);
  erFocus = $state<string | null>(null);
  tool = $state<ToolKind>('activity');
  dropped = $state.raw<Dropped | null>(null);
  dry = $state(false);
  notice = $state<string | null>(null);
  lastScope: RunScope | null = null;
  readonly data = new DataTab();

  constructor(connectionId: string, title: string, text = '') {
    this.connectionId = connectionId;
    this.title = title;
    this.text = text;
    paramsOf(this.id);
  }

  get running() {
    return this.runId !== null;
  }
}

export interface NamespacePending {
  relations: number;
  status: 'idle' | 'loading' | 'failed';
  error: string | null;
}

export class SessionState {
  info: SessionInfo;
  database: string;
  schema = $state.raw<Schema | null>(null);
  schemaError = $state<string | null>(null);
  searchPath = $state.raw<string[] | null>(null);
  failover = $state.raw<{ from: string; at: number } | null>(null);
  health = $state<Health>('ok');
  pending = $state.raw<Record<string, NamespacePending>>({});

  constructor(info: SessionInfo, database: string) {
    this.info = $state(info);
    this.database = $state(database);
  }
}

export const workspace = $state({
  sessions: {} as Record<string, SessionState>,
  extra: {} as Record<string, Record<string, SessionState>>,
  opening: {} as Record<string, boolean>,
  openErrors: {} as Record<string, ApiError | Error>,
  tabs: [] as Tab[],
  active: null as string | null,
  historyOpen: false,
  split: 0.45,
  widths: {} as Widths,
  epoch: 0,
});

export const activeTab = () => workspace.tabs.find((t) => t.id === workspace.active) ?? null;

export function sessionFor(connectionId: string, database: string | null = null) {
  const main = workspace.sessions[connectionId];
  if (!main) return null;
  return !database || database === main.database ? main : (workspace.extra[connectionId]?.[database] ?? null);
}

export const sessionOf = (tab: Tab) => sessionFor(tab.connectionId, tab.database);

export const tabKey = (tab: Tab) => (tab.database ? `${tab.connectionId}\u0000${tab.database}` : tab.connectionId);

export const databaseOf = (tab: Tab) => tab.database ?? sessionFor(tab.connectionId)?.database ?? null;

function profileName(id: string) {
  return connections.list.find((c) => c.id === id)?.name ?? id;
}

export async function openConnection(connectionId: string) {
  const existing = workspace.tabs.find((t) => t.connectionId === connectionId);
  if (sessionFor(connectionId) && existing) {
    workspace.active = existing.id;
    return;
  }
  if (await connect(connectionId)) newTab(connectionId);
}

export async function connect(connectionId: string): Promise<boolean> {
  if (sessionFor(connectionId)) return true;
  if (workspace.opening[connectionId]) return false;
  workspace.opening[connectionId] = true;
  delete workspace.openErrors[connectionId];
  try {
    const info = await api.sessionOpen(connectionId);
    workspace.sessions[connectionId] = new SessionState(info, info.server.database);
    watchTx(info);
    void loadVisibility(connectionId);
    void refreshSchema(connectionId);
    if (info.environment === 'production') void refreshAccess(info.session_id).catch(() => {});
    return true;
  } catch (e) {
    workspace.openErrors[connectionId] = e instanceof Error ? e : new Error(String(e));
    return false;
  } finally {
    delete workspace.opening[connectionId];
  }
}

const switching = new Map<string, Promise<boolean>>();

export async function ensureSession(connectionId: string, database: string | null): Promise<boolean> {
  if (!(await connect(connectionId))) return false;
  if (!database || sessionFor(connectionId, database)) return true;
  const key = `${connectionId}\u0000${database}`;
  const running = switching.get(key);
  if (running) return running;
  const work = switchTo(connectionId, database, key).finally(() => switching.delete(key));
  switching.set(key, work);
  return work;
}

async function switchTo(connectionId: string, database: string, key: string): Promise<boolean> {
  const main = sessionFor(connectionId);
  if (!main) return false;
  workspace.opening[key] = true;
  delete workspace.openErrors[key];
  try {
    const info = await api.sessionSwitch(main.info.session_id, database);
    (workspace.extra[connectionId] ??= {})[database] = new SessionState(info, database);
    watchTx(info);
    void refreshSchema(connectionId, database);
    if (info.environment === 'production') void refreshAccess(info.session_id).catch(() => {});
    return true;
  } catch (e) {
    workspace.openErrors[key] = e instanceof Error ? e : new Error(String(e));
    return false;
  } finally {
    delete workspace.opening[key];
  }
}

export async function useDatabase(tab: Tab, database: string): Promise<string | null> {
  if (tab.running || databaseOf(tab) === database) return null;
  const left = sessionOf(tab);
  const alone = left && left !== workspace.sessions[tab.connectionId] && !workspace.tabs.some((t) => t !== tab && sessionOf(t) === left);
  if (alone && !(await settleSessions([left.info.session_id], 'tab'))) return null;
  if (!(await ensureSession(tab.connectionId, database))) {
    const key = `${tab.connectionId}\u0000${database}`;
    const failed = workspace.openErrors[key]?.message ?? '';
    delete workspace.openErrors[key];
    return failed;
  }
  if (tab.view === 'data') {
    tab.table = null;
    tab.view = 'query';
  }
  tab.database = database;
  void prune(tab.connectionId);
  return null;
}

async function prune(connectionId: string) {
  const open = workspace.extra[connectionId];
  if (!open) return;
  for (const [database, s] of Object.entries(open)) {
    if (isWanted(connectionId, database) || workspace.tabs.some((t) => t.connectionId === connectionId && t.database === database)) continue;
    if (pending(txOf(s.info.session_id))) continue;
    delete open[database];
    forgetTx(s.info.session_id);
    await api.sessionClose(s.info.session_id).catch(() => {});
  }
}

function watchTx(info: SessionInfo) {
  trackSession(info);
  void refreshTx(info.session_id);
}

function sessionsOf(connectionId: string): SessionState[] {
  const main = workspace.sessions[connectionId];
  return [...(main ? [main] : []), ...Object.values(workspace.extra[connectionId] ?? {})];
}

const describeSession = (sessionId: string) => {
  const s = sessionById(sessionId);
  return { connection: profileName(s?.info.connection_id ?? ''), database: s?.database ?? '' };
};

function closedBy(list: Tab[]): string[] {
  const rest = workspace.tabs.filter((t) => !list.includes(t));
  return list.flatMap((t) => {
    if (!rest.some((r) => r.connectionId === t.connectionId)) return sessionsOf(t.connectionId).map((s) => s.info.session_id);
    const s = sessionOf(t);
    return s && !rest.some((r) => sessionOf(r) === s) && s !== workspace.sessions[t.connectionId] ? [s.info.session_id] : [];
  });
}

export function settleSessions(ids: string[], reason: CloseReason) {
  return settleBeforeClosing(waiting(ids, describeSession), reason);
}

export const waitingAll = () => waiting(allSessions().map((s) => s.info.session_id), describeSession);

export const releaseDatabase = prune;

export async function refreshSchema(connectionId: string, database: string | null = null) {
  const s = sessionFor(connectionId, database);
  if (!s) return;
  try {
    const opened = await api.schemaOpen(s.info.session_id);
    s.pending = Object.fromEntries(opened.pending.map((p) => [p.name, { relations: p.relations ?? 0, status: 'idle', error: null }]));
    s.schema = opened.schema;
    s.database = s.schema.current_database;
    s.schemaError = null;
  } catch (e) {
    s.schemaError = e instanceof Error ? e.message : String(e);
  }
  await refreshSearchPath(s);
}

export async function loadNamespace(s: SessionState, name: string) {
  const was = s.pending[name];
  if (!was || was.status === 'loading') return;
  s.pending = { ...s.pending, [name]: { ...was, status: 'loading', error: null } };
  try {
    const read = await api.schemaNamespace(s.info.session_id, name);
    if (s.pending[name]?.status !== 'loading' || !s.schema) return;
    const found = read.namespaces.find((n) => n.name === name);
    if (found) s.schema = { ...s.schema, namespaces: s.schema.namespaces.map((n) => (n.name === name ? found : n)) };
    const { [name]: _done, ...rest } = s.pending;
    s.pending = rest;
  } catch (e) {
    if (s.pending[name]) s.pending = { ...s.pending, [name]: { ...was, status: 'failed', error: e instanceof Error ? e.message : String(e) } };
  }
}

export async function refreshSearchPath(s: SessionState) {
  if (s.info.dialect !== 'postgres') return;
  const path = await api.searchPath(s.info.session_id).catch(() => null);
  if (path && path.join() !== s.searchPath?.join()) s.searchPath = path;
}

export function openTable(tab: Tab, namespace: string, name: string) {
  tab.table = { namespace, name };
  tab.view = 'data';
}

export function newTab(connectionId: string, text = '') {
  const n = workspace.tabs.filter((t) => t.connectionId === connectionId).length + 1;
  const tab = new Tab(connectionId, n === 1 ? profileName(connectionId) : `${profileName(connectionId)} · ${n}`, text);
  const from = activeTab();
  if (from?.connectionId === connectionId) tab.database = from.database;
  workspace.tabs = [...workspace.tabs, tab];
  workspace.active = tab.id;
  return tab;
}

export function queryTab(connectionId: string, sql: string) {
  const tab = newTab(connectionId, sql);
  void run(tab, { kind: 'script' });
  return tab;
}

export function duplicateTab(tab: Tab) {
  const copy = newTab(tab.connectionId, tab.text);
  copy.database = tab.database;
  copy.view = tab.view;
  if (tab.data.info) copy.table = { namespace: tab.data.info.namespace, name: tab.data.info.name };
  return copy;
}

export async function closeOthers(tab: Tab) {
  await closeTabs(workspace.tabs.filter((x) => x !== tab));
  workspace.active = tab.id;
}

export function showInEr(tab: Tab, namespace: string, name: string) {
  tab.erFocus = `${namespace}.${name}`;
  tab.view = 'schema';
}

const KEEP_ROWS = 5000;

async function removeTabs(list: Tab[]) {
  for (const t of list) if (t.runId) await api.runCancel(t.runId).catch(() => {});
  const gone = new Set(list);
  const at = workspace.tabs.findIndex((t) => t.id === workspace.active);
  const before = workspace.tabs.slice(0, Math.max(0, at)).filter((t) => gone.has(t)).length;
  const rest = workspace.tabs.filter((t) => !gone.has(t));
  if (at >= 0 && gone.has(workspace.tabs[at] as Tab)) workspace.active = (rest[at - before] ?? rest[at - before - 1])?.id ?? null;
  workspace.tabs = rest;
  for (const id of new Set(list.map((t) => t.connectionId))) {
    if (rest.some((t) => t.connectionId === id)) await prune(id);
    else await disconnect(id);
  }
}

export function park(t: Tab) {
  if (t.results.reduce((n, r) => n + r.count, 0) > KEEP_ROWS) t.results = [];
  t.runId = null;
}

export type Parked = { tab: Tab; index: number }[];

export const parkAll = (list: Tab[]): Parked => list.map((tab) => ({ tab, index: workspace.tabs.indexOf(tab) }));

export async function putBack(parked: Parked, active: string | null) {
  const tabs = [...workspace.tabs];
  for (const { tab, index } of [...parked].sort((a, b) => a.index - b.index)) tabs.splice(Math.min(index, tabs.length), 0, tab);
  workspace.tabs = tabs;
  if (active && tabs.some((t) => t.id === active)) workspace.active = active;
  for (const { tab } of parked) await ensureSession(tab.connectionId, tab.database);
}

export async function closeTabs(list: Tab[], label?: string) {
  if (list.length === 0) return;
  if (!(await settleSessions(closedBy(list), 'tab'))) return;
  const parked = parkAll(list);
  const active = workspace.active;
  const epoch = workspace.epoch;
  await removeTabs(list);
  for (const t of list) park(t);
  record({
    label: label ?? (list.length === 1 ? m.undo_tab_closed() : m.undo_tabs_closed({ count: list.length })),
    toast: true,
    valid: () => workspace.epoch === epoch,
    undo: () => putBack(parked, active),
    redo: () => removeTabs(list.filter((t) => workspace.tabs.includes(t))),
  });
}

export async function closeTab(id: string) {
  const tab = workspace.tabs.find((t) => t.id === id);
  if (tab) await closeTabs([tab]);
}

export async function closeConnection(connectionId: string) {
  const own = workspace.tabs.filter((t) => t.connectionId === connectionId);
  if (own.length) await closeTabs(own, m.undo_conn_closed({ name: profileName(connectionId) }));
  else await disconnect(connectionId);
}

export function renameTab(tab: Tab, title: string) {
  const before = tab.title;
  if (!title || title === before) return;
  tab.title = title;
  record({ label: m.undo_tab_renamed(), key: `tab:${tab.id}`, undo: () => void (tab.title = before), redo: () => void (tab.title = title), valid: () => workspace.tabs.includes(tab) });
}

export async function disconnect(connectionId: string) {
  const s = sessionFor(connectionId);
  if (!s) return;
  if (!(await settleSessions(sessionsOf(connectionId).map((x) => x.info.session_id), 'tab'))) return;
  for (const x of sessionsOf(connectionId)) forgetTx(x.info.session_id);
  for (const t of workspace.tabs.filter((t) => t.connectionId === connectionId)) if (t.runId) await api.runCancel(t.runId);
  workspace.tabs = workspace.tabs.filter((t) => t.connectionId !== connectionId);
  if (workspace.active && !workspace.tabs.some((t) => t.id === workspace.active)) workspace.active = workspace.tabs[0]?.id ?? null;
  const others = Object.values(workspace.extra[connectionId] ?? {});
  delete workspace.sessions[connectionId];
  delete workspace.extra[connectionId];
  forget(connectionId);
  for (const o of others) await api.sessionClose(o.info.session_id).catch(() => {});
  await api.sessionClose(s.info.session_id);
}

export function apply(tab: Tab, e: RunEvent) {
  switch (e.type) {
    case 'statement': {
      tab.results = [...tab.results, new ResultSet(e.index, e.sql, e.from, e.to, e.unsortable ?? null)];
      if (tab.results.length === 1) tab.activeResult = 0;
      break;
    }
    case 'columns': {
      const r = tab.results[e.index];
      if (r) {
        r.columns = e.columns;
        if (!tab.results[tab.activeResult]?.hasGrid) tab.activeResult = e.index;
      }
      break;
    }
    case 'rows': {
      const r = tab.results[e.index];
      if (r) {
        for (const row of e.rows) r.rows.push(row);
        r.count = r.rows.length;
      }
      break;
    }
    case 'documents': {
      const r = tab.results[e.index];
      if (r) {
        r.addDocuments(e.documents);
        if (!tab.results[tab.activeResult]?.hasGrid) tab.activeResult = e.index;
      }
      break;
    }
    case 'progress': {
      const r = tab.results[e.index];
      if (r) r.progress = e.progress;
      break;
    }
    case 'done': {
      const r = tab.results[e.index];
      if (r) r.done = doneOf(e);
      break;
    }
    case 'failed': {
      const r = tab.results[e.index];
      if (r) {
        r.error = { error: e.error, position: e.position, at: e.at, elapsed: e.elapsed_ms ?? 0 };
        tab.activeResult = e.index;
      }
      break;
    }
    case 'relocked': {
      const s = sessionOf(tab);
      if (s) void settle(s.info.session_id, true);
      break;
    }
    case 'reconnected':
    case 'lost': {
      tab.dropped = { index: e.index, write: e.write, transactionLost: e.type === 'reconnected' && e.transaction_lost, restored: e.type === 'reconnected', error: e.type === 'lost' ? describe(e.error) : null };
      const s = sessionOf(tab);
      if (s) s.health = e.type === 'lost' ? 'lost' : 'ok';
      if (s && e.type === 'reconnected' && s.info.environment === 'production') void refreshAccess(s.info.session_id).catch(() => {});
      break;
    }
    case 'preview': {
      const r = tab.results[e.index];
      if (r) r.preview = e.preview;
      break;
    }
    case 'implicit_commit':
      tab.notice = m.tx_implicit({ index: e.index + 1 });
      break;
    case 'transaction_lost':
      tab.notice = m.tx_lost_before();
      break;
    case 'transaction': {
      const s = sessionOf(tab);
      if (s) setTx(s.info.session_id, e.state);
      break;
    }
    case 'finished':
      tab.finished = { elapsed: e.elapsed_ms ?? 0, cancelled: e.cancelled };
      break;
  }
}

function allSessions(): SessionState[] {
  return [...Object.values(workspace.sessions), ...Object.values(workspace.extra).flatMap((x) => Object.values(x))];
}

const sessionById = (id: string) => allSessions().find((s) => s.info.session_id === id) ?? null;

export async function revive(s: SessionState): Promise<boolean> {
  if (s.health === 'ok') return true;
  const before = s.health;
  s.health = 'reconnecting';
  try {
    const info = await api.sessionReconnect(s.info.session_id);
    s.health = 'ok';
    const moved = movedMaster(s, info.server);
    const name = profileName(s.info.connection_id);
    if (info.transaction_lost) showToast(m.session_restored_tx({ name }), null, 'error', 12000);
    else if (before === 'lost' && !moved) showToast(m.session_restored({ name }));
    void refreshTx(s.info.session_id);
    void refreshSearchPath(s);
    if (s.info.environment === 'production') void refreshAccess(s.info.session_id).catch(() => {});
    return true;
  } catch (e) {
    s.health = before;
    showToast(m.session_lost({ name: profileName(s.info.connection_id), message: e instanceof Error ? e.message : String(e) }), null, 'error');
    return false;
  }
}

function movedMaster(s: SessionState, server: SessionInfo['server']): boolean {
  const before = sentinelMaster(s.info.server.database);
  const after = sentinelMaster(server.database);
  s.info.server = server;
  if (!before || !after || before.address === after.address) return false;
  s.failover = { from: before.address, at: Date.now() };
  showToast(m.sentinel_failover({ name: after.name, from: before.address, to: after.address }), null, 'info', 12000);
  return true;
}

export function onSessionEvent(e: SessionEvent) {
  const s = sessionById(e.session_id);
  if (!s) return;
  const name = profileName(s.info.connection_id);
  switch (e.type) {
    case 'reconnected': {
      s.health = 'ok';
      const moved = movedMaster(s, e.server);
      if (e.transaction_lost) showToast(m.session_restored_tx({ name }), null, 'error', 12000);
      else if (!moved) showToast(m.session_restored({ name }));
      void refreshTx(s.info.session_id);
      void refreshSearchPath(s);
      if (s.info.environment === 'production') void refreshAccess(s.info.session_id).catch(() => {});
      break;
    }
    case 'lost':
      s.health = 'lost';
      txGone(s.info.session_id);
      showToast(m.session_lost({ name, message: describe(e.error) }), { label: m.session_reconnect_now(), run: () => revive(s) }, 'error', 12000);
      break;
    case 'idle_closed':
      s.health = 'closed';
      txGone(s.info.session_id);
      if (e.transaction_lost) showToast(m.session_idle_closed_tx({ name }), null, 'error', 12000);
      else showToast(m.session_idle_closed({ name }));
      break;
  }
}

export function watchSessions() {
  void api.sessionWatch(onSessionEvent).catch(() => {});
}

export function rerunDropped(tab: Tab) {
  const d = tab.dropped;
  const r = d ? tab.results[d.index] : undefined;
  const last = tab.lastScope;
  if (!r || !last) return;
  const to = last.kind === 'selection' ? Math.max(last.from, last.to) : last.kind === 'script' ? tab.text.length : r.to;
  void run(tab, { kind: 'selection', from: r.from, to: Math.max(to, r.to) });
}

export async function run(tab: Tab, scope: RunScope, dryRun = false, keepGoing = prefs.value.editor.continue_on_error) {
  const s = sessionOf(tab);
  if (!s || tab.running) return;
  if (!(await revive(s))) return;
  const params = s.info.caps.params ? await paramsForRun(tab.id, tab.text, sqlOf(s.info), scope, dryRun).catch(() => null) : null;
  if (params === false) return;
  dryRun &&= s.info.caps.transactions;
  tab.lastScope = scope;
  tab.dropped = null;
  tab.dry = dryRun;
  tab.notice = null;
  const runId = crypto.randomUUID();
  tab.runId = runId;
  tab.results = [];
  tab.activeResult = 0;
  tab.finished = null;
  tab.failure = null;
  let asked = false;
  try {
    let confirm: RunConfirm_Deserialize = { risks: false, name: null };
    for (;;) {
      try {
        await api.run({ run_id: runId, session_id: s.info.session_id, text: tab.text, scope, confirm, dry_run: dryRun, params, continue_on_error: keepGoing }, (e) => apply(tab, e));
        break;
      } catch (e) {
        const next = e instanceof ApiError ? await confirmRun(s.info, e, confirm) : null;
        if (next === false) {
          tab.failure = m.run_not_confirmed();
          asked = true;
          return;
        }
        if (!next) throw e;
        confirm = next;
      }
    }
    if (tab.results.some((r) => r.done?.command && /^(CREATE|ALTER|DROP)$/.test(r.done.command))) void refreshSchema(tab.connectionId, tab.database);
  } catch (e) {
    tab.failure = e instanceof Error ? e.message : String(e);
    if (e instanceof ApiError && e.error.code === 'param') {
      const p = paramsOf(tab.id);
      tab.failure = paramFault(e.error.detail, nameLabel(e.error.detail.name, p.slots));
      p.fault = { name: e.error.detail.name, message: tab.failure };
    }
  } finally {
    tab.runId = null;
    if (s.info.environment === 'production') void settle(s.info.session_id);
    if (!asked) soundOf(tab);
    if (params) void refreshParams(tab.id, tab.text, sqlOf(s.info), scope);
    if (tab.results.some((r) => /search_path/i.test(r.sql))) void refreshSearchPath(s);
  }
}

const LONG_RUN_MS = 3000;
const unfocused = () => typeof document !== 'undefined' && !document.hasFocus();

function soundOf(tab: Tab) {
  if (tab.finished?.cancelled) return;
  if (tab.failure || tab.results.some((r) => r.error)) play('error');
  else if ((tab.finished?.elapsed ?? 0) > LONG_RUN_MS || unfocused()) play('done');
}

const RISK = {
  update_without_where: m.risk_update_without_where,
  delete_without_where: m.risk_delete_without_where,
  drop: m.risk_drop,
  truncate: m.risk_truncate,
  alter: m.risk_alter,
  flush: m.risk_flush,
  server: m.risk_server,
  full_scan: m.risk_full_scan,
};

export async function confirmRun(info: SessionInfo, e: ApiError, confirm: RunConfirm_Deserialize): Promise<RunConfirm_Deserialize | null | false> {
  const production = info.environment === 'production';
  const code = e.error.code;
  if (code !== 'risky' && code !== 'needs_confirmation') return null;
  if (production && !writesOpen(info.session_id)) throw new ApiError({ code: 'read_only' });
  const risks = code === 'risky' ? e.error.detail : [];
  const typed = await askConfirm({
    title: production ? m.confirm_run_prod() : m.confirm_run(),
    connection: connections.list.find((c) => c.id === info.connection_id)?.name ?? '',
    production,
    lines: risks.map((r) => RISK[r.risk]()),
    risks,
    note: production ? m.confirm_writes() : '',
    action: production ? m.confirm_run_prod_btn() : m.confirm_run_btn(),
  });
  if (typed === false) return false;
  return { risks: confirm.risks || code === 'risky', name: typed ?? confirm.name ?? null };
}

async function runSort(tab: Tab, r: ResultSet, keys: SortKey[], allRows = r.allRows) {
  const s = sessionOf(tab);
  if (!s || tab.running || (r.unsortable && keys.length)) return;
  if (!(await revive(s))) return;
  const runId = crypto.randomUUID();
  const before = r.sort;
  let fresh: Row[] | null = null;
  tab.runId = runId;
  r.sort = keys;
  r.sorting = true;
  r.sortError = null;
  const fail = (message: string) => {
    r.sort = before;
    r.sortError = message;
  };
  try {
    await api.run({ run_id: runId, session_id: s.info.session_id, text: r.sql, scope: { kind: 'script' }, confirm: { risks: false, name: null }, sort: keys, all_rows: allRows }, (e) => {
      if (e.type === 'rows') {
        if (!fresh) {
          fresh = [];
          r.rows = fresh;
        }
        for (const row of e.rows) fresh.push(row);
        r.count = fresh.length;
        r.rev++;
      } else if (e.type === 'done') {
        if (!fresh) {
          r.rows = [];
          r.count = 0;
        }
        r.done = doneOf(e);
        r.allRows = allRows;
      } else if (e.type === 'failed') fail(driverMessage(e.error));
      else if (e.type === 'finished' && e.cancelled && !fresh) r.sort = before;
    });
  } catch (e) {
    fail(e instanceof Error ? e.message : String(e));
  } finally {
    tab.runId = null;
    r.sorting = false;
    if (s.info.environment === 'production') void settle(s.info.session_id);
  }
}

export function loadAll(tab: Tab, r: ResultSet) {
  return runSort(tab, r, r.sort, true);
}

export async function sortResult(tab: Tab, r: ResultSet, keys: SortKey[]) {
  const before = r.sort;
  await runSort(tab, r, keys);
  if (r.sortError || r.sort === before) return;
  const after = r.sort;
  const show = (to: SortKey[]) => {
    workspace.active = tab.id;
    return runSort(tab, r, to);
  };
  record({ label: m.undo_sorted(), key: `rsort:${tab.id}:${r.index}`, undo: () => show(before), redo: () => show(after), valid: () => tab.results.includes(r) });
}

export interface DataFilters {
  where: string;
  quick: QuickFilter[];
  order: Order[];
}

const filtersOf = (d: DataTab): DataFilters => ({ where: d.where, quick: d.quick, order: d.order });

export function changeDataView(tab: Tab, session: SessionInfo, next: Partial<DataFilters>, label: string) {
  const d = tab.data;
  const info = d.info;
  const before = filtersOf(d);
  Object.assign(d, next);
  const after = filtersOf(d);
  void d.reload(session);
  const show = (to: DataFilters) => {
    const live = sessionOf(tab);
    if (!live) throw new Error(m.undo_no_session());
    workspace.active = tab.id;
    tab.view = 'data';
    Object.assign(d, to);
    void d.reload(live.info);
  };
  record({
    label,
    key: `view:${tab.id}`,
    undo: () => show(before),
    redo: () => show(after),
    valid: () => workspace.tabs.includes(tab) && !!info && d.info?.namespace === info.namespace && d.info.name === info.name,
  });
}

export function discardChanges(tab: Tab) {
  const d = tab.data;
  const old = d.pending;
  if (!old || old.empty) return;
  d.pending = new Pending(old.shape);
  d.message = null;
  d.touch();
  record({
    label: m.undo_discarded(),
    toast: true,
    undo: () => {
      workspace.active = tab.id;
      d.pending = old;
      d.touch();
    },
    redo: () => {
      d.pending = new Pending(old.shape);
      d.touch();
    },
    valid: () => workspace.tabs.includes(tab) && d.info !== null,
  });
}

undoFirst(() => {
  const t = activeTab();
  if (t?.view !== 'data' || !t.data.pending?.undo()) return false;
  t.data.touch();
  return true;
});

redoFirst(() => {
  const t = activeTab();
  if (t?.view !== 'data' || !t.data.pending?.redo()) return false;
  t.data.touch();
  return true;
});

export async function cancel(tab: Tab) {
  if (tab.runId) await api.runCancel(tab.runId);
}
