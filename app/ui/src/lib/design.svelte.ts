import { sqlOf } from './family';
import { m } from '../paraglide/messages';
import { api, ApiError, driverMessage } from './api';
import type { DesignScript_Serialize as DesignScript, RunConfirm_Deserialize, SavedDesign, SavedDesign_Serialize } from './bindings';
import { copyText } from './contextmenu.svelte';
import { blankDesign, ENGINE_NAME, engineOf, hasDryRun, scriptText, type DesignEngine, type TableDesign } from './design';
import { queryOn } from './explorer.svelte';
import { settle } from './guard.svelte';
import type { Command } from './palette.svelte';
import { play } from './sound';
import { showToast } from './toast.svelte';
import { commitTx, refreshTx, rollbackTx, txOf } from './tx.svelte';
import { activeTab, closeTab, confirmRun, connect, databaseOf, newTab, refreshSchema, sessionFor, useDatabase, workspace, type Tab } from './workspace.svelte';

const EMPTY: DesignScript = { statements: [], notes: [], rebuild: false };

export type DesignSection = 'columns' | 'indexes' | 'foreign_keys' | 'checks';

export class Design {
  readonly connectionId: string;
  readonly database: string;
  readonly namespace: string;
  readonly engine: DesignEngine;
  origin = $state<string | null>(null);
  before = $state.raw<TableDesign | null>(null);
  after = $state<TableDesign>(blankDesign('postgres', ''));
  script = $state.raw<DesignScript>(EMPTY);
  section = $state<DesignSection>('columns');
  loading = $state(false);
  failure = $state<string | null>(null);
  applying = $state(false);
  error = $state<string | null>(null);
  trial = $state(false);
  pending = $state(false);
  draft: TableDesign | null = null;
  private seq = 0;

  constructor(connectionId: string, database: string, namespace: string, engine: DesignEngine, origin: string | null) {
    this.connectionId = connectionId;
    this.database = database;
    this.namespace = namespace;
    this.engine = engine;
    this.origin = origin;
    this.after = blankDesign(engine, namespace);
  }

  get session() {
    return sessionFor(this.connectionId, this.database);
  }

  get text() {
    return scriptText(this.script.statements);
  }

  get blocked() {
    return this.script.notes.some((n) => n.code === 'invalid' || n.code === 'trigger_blocks');
  }

  async load() {
    this.failure = null;
    this.error = null;
    if (!this.origin) {
      this.before = null;
      this.after = blankDesign(this.engine, this.namespace);
      return;
    }
    const s = this.session;
    if (!s) return;
    this.loading = true;
    try {
      const d = await api.tableDesign(s.info.session_id, this.namespace, this.origin);
      this.before = d;
      this.after = structuredClone(d);
    } catch (e) {
      this.failure = e instanceof Error ? e.message : String(e);
    } finally {
      this.loading = false;
    }
  }

  async wake() {
    if (!this.pending || !this.session) return;
    this.pending = false;
    await this.load();
    if (this.draft && !this.failure) this.after = this.draft;
    this.draft = null;
  }

  saved(): SavedDesign_Serialize {
    const after = JSON.stringify($state.snapshot(this.after));
    const edited = this.pending ? this.draft && JSON.stringify(this.draft) : after !== JSON.stringify(this.before ? $state.snapshot(this.before) : blankDesign(this.engine, this.namespace)) ? after : null;
    return { namespace: this.namespace, engine: this.engine, origin: this.origin, draft: edited || null };
  }

  reset() {
    this.error = null;
    this.after = this.before ? structuredClone($state.snapshot(this.before) as TableDesign) : blankDesign(this.engine, this.namespace);
  }

  async refreshScript() {
    const n = ++this.seq;
    const out = await api.tableDesignScript(this.engine, this.before, $state.snapshot(this.after) as TableDesign);
    if (n === this.seq) this.script = out;
  }

  async copy() {
    await copyText(this.text);
    showToast(m.dz_copied());
  }

  async openInEditor() {
    await queryOn(this.connectionId, this.database, this.text, false);
  }

  async apply() {
    const s = this.session;
    const text = this.text;
    if (!s || !text || this.applying || this.blocked) return;
    const sid = s.info.session_id;
    const dry = hasDryRun(this.engine);
    this.applying = true;
    this.error = null;
    let failure: string | null = null;
    let confirm: RunConfirm_Deserialize = { risks: false, name: null };
    try {
      for (;;) {
        try {
          await api.run({ run_id: crypto.randomUUID(), session_id: sid, text, scope: { kind: 'script' }, confirm, dry_run: dry }, (e) => {
            if (e.type === 'failed' && !failure) failure = driverMessage(e.error);
          });
          break;
        } catch (e) {
          const next = e instanceof ApiError ? await confirmRun(s.info, e, confirm) : null;
          if (next === false) return;
          if (!next) throw e;
          confirm = next;
        }
      }
      await refreshTx(sid);
      if (failure) {
        if (dry && txOf(sid)?.open) await rollbackTx(sid);
        this.error = failure;
        play('error');
        return;
      }
      if (dry && txOf(sid)?.open) {
        this.trial = true;
        return;
      }
      await this.done();
    } catch (e) {
      this.error = e instanceof Error ? e.message : String(e);
      play('error');
    } finally {
      this.applying = false;
      if (s.info.environment === 'production') void settle(sid);
    }
  }

  async confirmTrial() {
    const s = this.session;
    if (!s) return;
    if (await commitTx(s.info.session_id)) await this.done();
  }

  async cancelTrial() {
    const s = this.session;
    if (!s) return;
    if (await rollbackTx(s.info.session_id)) {
      this.trial = false;
      showToast(m.dz_rolled_back());
    }
  }

  private async done() {
    this.trial = false;
    const name = this.after.name;
    const created = !this.origin;
    this.origin = name;
    await refreshSchema(this.connectionId, this.database);
    await this.load();
    play('applied');
    showToast(created ? m.dz_created({ name }) : m.dz_applied({ name }));
    const tab = workspace.tabs.find((t) => designs[t.id] === this);
    if (tab) tab.title = m.dz_tab_edit({ name });
  }
}

export const designs: Record<string, Design> = $state({});

export const designOf = (tab: Tab | null | undefined) => (tab ? (designs[tab.id] ?? null) : null);

export function restoreDesign(tab: Tab, saved: SavedDesign) {
  const engine = (Object.keys(ENGINE_NAME) as DesignEngine[]).find((x) => x === saved.engine) ?? 'postgres';
  const d = new Design(tab.connectionId, tab.database ?? '', saved.namespace, engine, saved.origin ?? null);
  d.pending = true;
  try {
    d.draft = saved.draft ? (JSON.parse(saved.draft) as TableDesign) : null;
  } catch {
    d.draft = null;
  }
  designs[tab.id] = d;
}

function prune() {
  for (const id of Object.keys(designs)) if (!workspace.tabs.some((t) => t.id === id)) delete designs[id];
}

export async function openDesigner(connectionId: string, database: string, namespace: string, name: string | null) {
  prune();
  const existing = name ? workspace.tabs.find((t) => {
    const d = designs[t.id];
    return d && d.connectionId === connectionId && d.database === database && d.namespace === namespace && d.origin === name;
  }) : undefined;
  if (existing) {
    workspace.active = existing.id;
    return;
  }
  if (!(await connect(connectionId))) return;
  const tab = newTab(connectionId);
  if (databaseOf(tab) !== database) {
    const failed = await useDatabase(tab, database);
    if (failed !== null) {
      await closeTab(tab.id);
      showToast(failed || m.dbs_open_failed(), null, 'error');
      return;
    }
  }
  const s = sessionFor(connectionId, database);
  if (!s) return;
  const engine = engineOf(sqlOf(s.info), s.info.server.product);
  if (!engine) {
    await closeTab(tab.id);
    showToast(m.dz_unsupported(), null, 'error');
    return;
  }
  tab.title = name ? m.dz_tab_edit({ name }) : m.dz_tab_new();
  const d = new Design(connectionId, database, namespace, engine, name);
  designs[tab.id] = d;
  await d.load();
}

function dataTable(tab: Tab | null) {
  const info = tab?.view === 'data' && !designOf(tab) ? tab.data.info : null;
  if (!tab || !info || info.view) return null;
  return { tab, namespace: info.namespace, name: info.name };
}

function defaultNamespace(tab: Tab) {
  const s = sessionFor(tab.connectionId, tab.database);
  const d = databaseOf(tab) ?? '';
  const spaces = s?.schema?.namespaces.map((n) => n.name) ?? [];
  const preferred = (s?.info ? sqlOf(s.info) : undefined) === 'postgres' ? 'public' : (s?.info ? sqlOf(s.info) : undefined) === 'mssql' ? 'dbo' : (s?.info ? sqlOf(s.info) : undefined) === 'mysql' ? d : 'main';
  return spaces.includes(preferred) ? preferred : (spaces[0] ?? preferred);
}

export function designCommands(): Command[] {
  const group = m.pal_group_design;
  return [
    {
      id: 'schema.design_edit',
      label: m.dz_edit_table,
      group,
      when: () => !!dataTable(activeTab()),
      run: () => {
        const x = dataTable(activeTab());
        if (x) void openDesigner(x.tab.connectionId, databaseOf(x.tab) ?? '', x.namespace, x.name);
      },
    },
    {
      id: 'schema.design_new',
      label: m.dz_new_table,
      group,
      when: () => {
        const t = activeTab();
        return !!t && !!sessionFor(t.connectionId, t.database);
      },
      run: () => {
        const t = activeTab();
        if (t) void openDesigner(t.connectionId, databaseOf(t) ?? '', defaultNamespace(t), null);
      },
    },
  ];
}
