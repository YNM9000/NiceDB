import { m } from '../paraglide/messages';
import { api, ApiError, driverMessage, type Progress, type SessionInfo } from './api';
import type { KillMode } from './bindings';
import type { DataExportSource, FileFormat, RunConfirm_Deserialize } from './bindings';
import { connections } from './connections.svelte';
import { askConfirm, settle, writesOpen } from './guard.svelte';
import type { ToolKind } from './tools';
import { activeTab, confirmRun, openConnection, sessionFor, type Tab } from './workspace.svelte';

export interface ExportAsk {
  connectionId: string;
  database?: string | null;
  source: DataExportSource;
  label: string;
  expected: number | null;
  format?: FileFormat;
}

export interface ImportAsk {
  connectionId: string;
  database?: string | null;
  table: { namespace: string; name: string } | null;
}

export const toolsUi = $state({ export: null as ExportAsk | null, import: null as ImportAsk | null, compareTarget: null as { tab: string; target: string } | null });

export const askExport = (a: ExportAsk) => (toolsUi.export = a);
export const askImport = (a: ImportAsk) => (toolsUi.import = a);

export async function openTool(connectionId: string, tool: ToolKind) {
  await openConnection(connectionId);
  const tab = activeTab();
  if (tab?.connectionId !== connectionId) return;
  tab.view = 'tools';
  tab.tool = tool;
}

const rowSeeds = new Map<string, string>();

export function compareResult(tab: Tab, sql: string) {
  rowSeeds.set(tab.id, sql);
  tab.view = 'tools';
  tab.tool = 'rows';
}

export function compareWith(tab: Tab, target: string) {
  toolsUi.compareTarget = { tab: tab.id, target };
  tab.view = 'tools';
  tab.tool = 'compare';
}

export function takeRowSeed(tabId: string): string | null {
  const sql = rowSeeds.get(tabId) ?? null;
  rowSeeds.delete(tabId);
  return sql;
}

const connectionName = (info: SessionInfo) => connections.list.find((c) => c.id === info.connection_id)?.name ?? '';

export class Job {
  id = $state<string | null>(null);
  progress = $state.raw<Progress | null>(null);
  started = $state(0);

  get running() {
    return this.id !== null;
  }

  async run<T>(work: (id: string, onProgress: (p: Progress) => void) => Promise<T>): Promise<T> {
    const id = crypto.randomUUID();
    this.id = id;
    this.progress = null;
    this.started = Date.now();
    try {
      return await work(id, (p) => {
        if (this.id === id) this.progress = p;
      });
    } finally {
      if (this.id === id) this.id = null;
    }
  }

  async cancel() {
    if (this.id) await api.jobCancel(this.id);
  }
}

export async function confirmToolWrite(info: SessionInfo, title: string, lines: string[], action: string, note: string, ask = true): Promise<string | null | false> {
  const production = info.environment === 'production';
  if (production && !writesOpen(info.session_id)) throw new ApiError({ code: 'read_only' });
  if (!production && !ask) return null;
  return askConfirm({
    title,
    connection: connectionName(info),
    production,
    lines,
    risks: [],
    note,
    action,
  });
}

export const errorText = (e: unknown) => (e instanceof Error ? e.message : String(e));

export async function runScript(connectionId: string, sql: string): Promise<string | null> {
  const s = sessionFor(connectionId);
  if (!s) return m.err_busy();
  let confirm: RunConfirm_Deserialize = { risks: false, name: null };
  let failure: string | null = null;
  try {
    for (;;) {
      try {
        await api.run({ run_id: crypto.randomUUID(), session_id: s.info.session_id, text: sql, scope: { kind: 'script' }, confirm }, (e) => {
          if (e.type === 'failed' && !failure) failure = driverMessage(e.error);
        });
        return failure;
      } catch (e) {
        const next = e instanceof ApiError ? await confirmRun(s.info, e, confirm) : null;
        if (next === false) return m.run_not_confirmed();
        if (!next) throw e;
        confirm = next;
      }
    }
  } catch (e) {
    return errorText(e);
  } finally {
    if (s.info.environment === 'production') void settle(s.info.session_id);
  }
}

export async function killServerSession(info: SessionInfo, target: { id: string; query: string | null }, mode: KillMode): Promise<string | null> {
  try {
    const cancel = mode === 'cancel';
    const title = cancel ? m.tool_kill_cancel_title({ id: target.id }) : m.tool_kill_close_title({ id: target.id });
    const query = target.query ? (target.query.length > 200 ? `${target.query.slice(0, 200)}…` : target.query) : m.tool_kill_no_query();
    const typed = await confirmToolWrite(info, title, [query], cancel ? m.tool_kill_cancel() : m.tool_kill_close(), cancel ? m.tool_kill_cancel_note() : m.tool_kill_close_note());
    if (typed === false) return null;
    await api.serverKill(info.session_id, target.id, mode, typed);
    return cancel ? m.tool_kill_cancelled({ id: target.id }) : m.tool_kill_closed({ id: target.id });
  } catch (e) {
    return errorText(e);
  }
}
