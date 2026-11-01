<script lang="ts">
  import { ICON } from '../lib/icons';
  import { namesOf, sqlOf } from '../lib/family';
  import { sqlgenCommands } from '../lib/sqlgen.svelte';
  import { untrack } from 'svelte';
  import { FlaskConical, History, NotebookPen, Play, Plus, Route, Save, Square, Wand, X } from '@lucide/svelte';
  import { keys, openMenu } from '../lib/contextmenu.svelte';
  import { writesOpen } from '../lib/guard.svelte';
  import { explain } from '../lib/plan.svelte';
  import { activeTab, cancel, closeOthers, closeTab, closeTabs, databaseOf, duplicateTab, ensureSession, newTab, renameTab, revive, run, sessionOf, tabKey, workspace, type Tab } from '../lib/workspace.svelte';
  import { openSave, panel, toggleHistory, togglePanel } from '../lib/panel.svelte';
  import { currentBindings } from '../lib/prefs.svelte';
  import { matches } from '../lib/shortcuts';
  import { m } from '../paraglide/messages';
  import NotesPanel from './NotesPanel.svelte';
  import SaveScriptDialog from './SaveScriptDialog.svelte';
  import { visibilityOf } from '../lib/dbs.svelte';
  import { visibleSchema } from '../lib/visibility';
  import WriteLock from './WriteLock.svelte';
  import DataView from './DataView.svelte';
  import KeyBrowser from './KeyBrowser.svelte';
  import { keyLookup } from '../lib/keys.svelte';
  import Editor from './Editor.svelte';
  import SchemaView from './SchemaView.svelte';
  import ToolDialogs from './ToolDialogs.svelte';
  import ToolsView from './ToolsView.svelte';
  import HistoryPanel from './HistoryPanel.svelte';
  import ResultsPanel from './ResultsPanel.svelte';
  import ParamsBar from './ParamsBar.svelte';
  import { paramsIn, paramsOf, refreshParams } from '../lib/params.svelte';
  import TxBar from './TxBar.svelte';
  import { api } from '../lib/api';
  import { describer } from '../lib/colinfo';
  import { txOf } from '../lib/tx.svelte';
  import TxClose from './TxClose.svelte';
  import { paletteHost, registerCommands } from '../lib/palette.svelte';
  import { workspaceCommands } from '../lib/palette-commands';
  import { moreCommands } from '../lib/palette-more';
  import { designCommands, designOf } from '../lib/design.svelte';
  import TableDesigner from './TableDesigner.svelte';
  import DesignBar from './DesignBar.svelte';

  const tab = $derived(activeTab());
  const design = $derived(designOf(tab));
  const session = $derived(tab ? sessionOf(tab) : null);
  const describe = $derived(session?.info.dialect && session.schema ? describer(session.info.session_id, session.schema, api.tableDesign) : null);
  const editorSchema = $derived(session?.schema && tab ? visibleSchema(session.schema, visibilityOf(tab.connectionId)) : null);
  const multi = (t: Tab) => (workspace.sessions[t.connectionId]?.schema?.databases.length ?? 0) > 1;
  const production = $derived(session?.info.environment === 'production');
  const unlocked = $derived(!!session && production && writesOpen(session.info.session_id));
  let editor = $state<Editor | null>(null);
  let body = $state<HTMLDivElement>();
  let renaming = $state<string | null>(null);

  const tabProd = (t: Tab) => sessionOf(t)?.info.environment === 'production';
  const tabEdits = (t: Tab) => (void t.data.rev, !!t.data.pending && !t.data.pending.empty);
  const tabTx = (t: Tab) => !!txOf(sessionOf(t)?.info.session_id)?.open;
  const tabNote = (t: Tab) =>
    [t.title, multi(t) && databaseOf(t), t.running && m.tab_running(), tabEdits(t) && m.tab_dirty(), tabTx(t) && m.tx_open(), tabProd(t) && m.env_production()].filter(Boolean).join(' · ');

  const sideHistory = $derived(workspace.historyOpen && tab?.view === 'query' && !design && !panel.open);

  function saveCurrent() {
    if (!tab || !editor || tab.view !== 'query') return;
    const sql = editor.selected() || editor.content();
    openSave(sql, tab.connectionId, null, paramsIn(tab.id, sql));
  }

  $effect(() => {
    const onkey = (e: KeyboardEvent) => {
      if (e.defaultPrevented || document.querySelector('dialog[open]')) return;
      const b = currentBindings();
      const act = matches(e, b.save_script) ? saveCurrent : matches(e, b.panel_notes) ? () => togglePanel() : matches(e, b.panel_history) ? toggleHistory : matches(e, b.new_tab) ? () => tab && newTab(tab.connectionId) : null;
      if (!act) return;
      e.preventDefault();
      act();
    };
    window.addEventListener('keydown', onkey);
    return () => window.removeEventListener('keydown', onkey);
  });

  $effect(() => registerCommands(workspaceCommands({ tab: activeTab, editor: () => editor, save: saveCurrent, explain: explainHere })));
  $effect(() => registerCommands(sqlgenCommands(activeTab)));
  $effect(() => registerCommands(designCommands()));
  $effect(() => registerCommands(moreCommands({ tab: activeTab, editorText: () => (activeTab()?.view === 'query' ? (editor?.content() ?? null) : null) })));
  $effect(() => {
    paletteHost.editor = tab?.view === 'query' && !design ? editor : null;
    return () => (paletteHost.editor = null);
  });

  $effect(() => {
    if (!tab) return;
    const key = tabKey(tab);
    if (!sessionOf(tab) && !workspace.opening[key] && !workspace.openErrors[key]) void ensureSession(tab.connectionId, tab.database);
  });

  $effect(() => {
    if (tab && (tab.view !== 'query' || design) && session?.health === 'closed') void revive(session);
  });

  $effect(() => {
    if (design?.pending && session) void design.wake();
  });

  function tabMenu(e: MouseEvent, t: Tab) {
    const i = workspace.tabs.indexOf(t);
    openMenu(e, [
      { label: m.tab_rename(), icon: ICON.rename, shortcut: 'F2', run: () => (renaming = t.id) },
      { label: m.tab_duplicate(), icon: ICON.duplicate, run: () => duplicateTab(t) },
      'separator',
      { label: m.tab_close(), icon: ICON.closeTab, run: () => closeTab(t.id) },
      { label: m.tab_close_others(), disabled: workspace.tabs.length < 2, run: () => closeOthers(t) },
      {
        label: m.tab_close_right(),
        disabled: i === workspace.tabs.length - 1,
        run: () => closeTabs(workspace.tabs.slice(i + 1)),
      },
    ]);
  }

  function renameKey(e: KeyboardEvent, t: Tab) {
    if (e.key === 'Enter') commitRename(e.currentTarget as HTMLInputElement, t);
    if (e.key === 'Escape') renaming = null;
  }

  function commitRename(input: HTMLInputElement, t: Tab) {
    renameTab(t, input.value.trim());
    renaming = null;
  }

  function autoselect(node: HTMLInputElement) {
    node.focus();
    node.select();
  }

  function runningRange(t: Tab): [number, number] | null {
    if (!t.running) return null;
    const r = t.results.at(-1);
    return r && !r.done && !r.error ? [r.from, r.to] : null;
  }

  function errorRange(t: Tab): [number, number] | null {
    const at = t.results.find((r) => r.error)?.error?.at;
    if (at === null || at === undefined) return null;
    let end = at;
    while (end < t.text.length && /[\p{L}\p{N}_$"]/u.test(t.text[end] ?? '')) end++;
    return [at, Math.max(end, at + 1)];
  }

  function drag(e: PointerEvent) {
    const handle = e.currentTarget as HTMLElement;
    handle.setPointerCapture(e.pointerId);
    if (!body) return;
    const rect = body.getBoundingClientRect();
    const move = (ev: PointerEvent) => (workspace.split = Math.min(0.8, Math.max(0.15, (ev.clientY - rect.top) / rect.height)));
    const up = () => {
      handle.removeEventListener('pointermove', move);
      handle.removeEventListener('pointerup', up);
    };
    handle.addEventListener('pointermove', move);
    handle.addEventListener('pointerup', up);
  }

  let paramTimer: ReturnType<typeof setTimeout> | undefined;
  function watchParams() {
    clearTimeout(paramTimer);
    paramTimer = setTimeout(() => {
      if (tab && session && editor && session.info.caps.params) void refreshParams(tab.id, tab.text, sqlOf(session.info), editor.runScope(false));
    }, 140);
  }
  $effect(() => {
    if (tab?.view === 'query' && session && editor) untrack(watchParams);
  });
  const spots = $derived(tab ? paramsOf(tab.id).slots.flatMap((s) => s.spots.map((x) => [x.from, x.to] as const)) : []);

  function runFromParams(t: Tab) {
    const pending = paramsOf(t.id).pending;
    if (pending) void run(t, pending.scope, pending.dry);
    else if (editor) void run(t, editor.runScope(false));
  }

  function explainHere(t: Tab) {
    if (!editor) return;
    workspace.split = Math.min(workspace.split, 0.24);
    void explain(t, editor.runScope(false));
  }

  let tabBar: HTMLElement | undefined = $state();

  function tabWheel(e: WheelEvent) {
    if (!tabBar || e.deltaX || tabBar.scrollWidth <= tabBar.clientWidth) return;
    tabBar.scrollLeft += e.deltaY;
    e.preventDefault();
  }

  $effect(() => {
    void workspace.active;
    void workspace.tabs.length;
    tabBar?.querySelector('.tab.active')?.scrollIntoView({ block: 'nearest', inline: 'nearest' });
  });
  $effect(() => {
    const bar = tabBar;
    if (!bar) return;
    const keep = new ResizeObserver(() => bar.querySelector('.tab.active')?.scrollIntoView({ block: 'nearest', inline: 'nearest' }));
    keep.observe(bar);
    return () => keep.disconnect();
  });

  function resizeKey(e: KeyboardEvent) {
    if (e.key === 'ArrowUp') workspace.split = Math.max(0.15, workspace.split - 0.05);
    if (e.key === 'ArrowDown') workspace.split = Math.min(0.8, workspace.split + 0.05);
  }
</script>

<div class="workspace" class:with-history={sideHistory}
  class:with-panel={panel.open} class:prod={production} class:writes-open={unlocked} class:bare={!!tab && !!session && !design && tab.view !== 'query'}>
  <div class="tabstrip">
  <div class="tabs" role="tablist" aria-label={m.tabs_label()} bind:this={tabBar} onwheel={tabWheel}>
    {#each workspace.tabs as t (t.id)}
      <div class="tab" class:active={t.id === workspace.active} class:busy={t.running} class:prod={tabProd(t)} class:dirty={tabEdits(t) || tabTx(t)} oncontextmenu={(e) => tabMenu(e, t)} onmousedown={(e) => e.button === 1 && e.preventDefault()} onauxclick={(e) => e.button === 1 && void closeTab(t.id)} role="none">
        {#if renaming === t.id}
          <input class="tab-rename" value={t.title} aria-label={m.tab_rename()} use:autoselect onkeydown={(e) => renameKey(e, t)} onblur={(e) => commitRename(e.currentTarget, t)} />
        {:else}
          <button
            type="button"
            role="tab"
            aria-selected={t.id === workspace.active}
            data-tip={tabNote(t)}
            onclick={() => {
              workspace.active = t.id;
            }}
            ondblclick={() => (renaming = t.id)}
            onkeydown={(e) => e.key === 'F2' && (renaming = t.id)}
            >{t.title}</button
          >
          {#if multi(t) && databaseOf(t)}<span class="tab-db" data-tip={databaseOf(t)}>{databaseOf(t)}</span>{/if}
        {/if}
        <button class="tab-close" type="button" aria-label={m.tab_close()} onclick={() => void closeTab(t.id)}><X size={12} strokeWidth={2} /></button>
      </div>
    {/each}
  </div>
  {#if tab}
    <button class="icon-btn tab-new" type="button" aria-label={m.tab_new()} onclick={() => tab && newTab(tab.connectionId)}><Plus size={14} strokeWidth={1.8} /></button>
  {/if}
  {#if tab && session}
    <div class="tab-tools">
      {#if production}<WriteLock sessionId={session.info.session_id} />{/if}
      {#if session.info.caps.transactions}<TxBar sessionId={session.info.session_id} running={tab.running} />{/if}
      <button class="icon-btn panel-toggle" type="button" aria-pressed={panel.open} aria-label={m.panel_title()} data-tip={m.panel_title()} onclick={() => togglePanel()}>
        <NotebookPen size={14} strokeWidth={1.8} aria-hidden="true" />
      </button>
    </div>
  {/if}
  </div>

  {#if tab && session}
    {#if design || tab.view === 'query'}
    <div class="toolbar">
      {#if design}
        <DesignBar {design} />
      {:else if tab.running}
        {@const reach = session.info.caps.cancel}
        <button
          class="btn danger"
          type="button"
          data-tip={reach === 'none' ? m.run_cancel_none() : reach === 'between_fetches' ? m.run_cancel_fetches() : undefined}
          onclick={() => void cancel(tab)}
        >
          <Square size={13} strokeWidth={2} aria-hidden="true" />
          {m.run_cancel()} <kbd>Esc</kbd>
        </button>
      {:else}
        <button class="btn primary" type="button" onclick={() => editor && void run(tab, editor.runScope(false))}>
          <Play size={13} strokeWidth={2} aria-hidden="true" />
          {m.run_statement()} <kbd>Ctrl ↵</kbd>
        </button>
        <button class="btn" type="button" onclick={() => editor && void run(tab, editor.runScope(true))}>{m.run_script()} <kbd>⇧ Ctrl ↵</kbd></button>
        {#if session.info.caps.explain}
        <button class="btn compactable" type="button" data-tip={m.plan_explain_hint()} aria-label={m.plan_explain()} onclick={() => explainHere(tab)}>
          <Route size={13} strokeWidth={1.8} aria-hidden="true" />
          {m.plan_explain()}
        </button>
        {/if}
        {#if session.info.caps.transactions}
        <button class="btn compactable dry-run" type="button" data-tip={currentBindings().dry_run ? `${m.tx_dry_hint()} · ${keys(currentBindings().dry_run)}` : m.tx_dry_hint()} aria-label={m.tx_dry()} onclick={() => editor && void run(tab, editor.runScope(false), true)}>
          <FlaskConical size={13} strokeWidth={1.8} aria-hidden="true" />
          {m.tx_dry()}
        </button>
        {/if}
      {/if}
      {#if tab.view === 'query' && !design}
        <button class="btn ghost" type="button" data-tip={m.run_format()} onclick={() => editor?.format()}>
          <Wand size={13} strokeWidth={1.8} aria-hidden="true" />
          {m.run_format()}
        </button>
        <button class="btn ghost compactable save-script" type="button" data-tip={m.panel_scripts_save()} aria-label={m.panel_scripts_save()} onclick={saveCurrent}>
          <Save size={13} strokeWidth={1.8} aria-hidden="true" />
          {m.panel_scripts_save_short()}
        </button>
        <span class="spacer"></span>
        <button class="btn ghost" type="button" data-tip={m.run_history()} aria-pressed={sideHistory} onclick={toggleHistory}>
          <History size={13} strokeWidth={1.8} aria-hidden="true" />
          {m.run_history()}
        </button>
      {/if}
    </div>
    {/if}

    {#if design}
      {#key tab.id}<TableDesigner {design} />{/key}
    {:else if tab.view === 'data' && session.info.caps.results === 'keys'}
      <KeyBrowser {tab} {session} />
    {:else if tab.view === 'data'}
      <DataView {tab} {session} />
    {:else if tab.view === 'schema'}
      <SchemaView {tab} {session} />
    {:else if tab.view === 'tools'}
      <ToolsView {tab} {session} />
    {:else}

    <div class="work-body" bind:this={body} style:grid-template-rows="{workspace.split}fr 7px {1 - workspace.split}fr">
      <div class="editor-pane">
        {#key tab.id}
          <Editor
            bind:this={editor}
            text={tab.text}
            dialect={session.info.dialect ?? null}
            native={session.info.caps.language.kind === 'sql' ? 'mql' : session.info.caps.language.kind}
            keyNames={session.info.caps.family === 'key_value' ? keyLookup(session.info.session_id) : null}
            schema={editorSchema}
            running={runningRange(tab)}
            error={errorRange(tab)}
            onchange={(text) => (tab.text = text)}
            onrun={(scope, dry, keepGoing) => void run(tab, scope, dry, keepGoing)}
            product={session.info.server.product}
            searchPath={session.searchPath}
            describe={describe}
            oncancel={() => {
              if (!tab.running) return false;
              void cancel(tab);
              return true;
            }}
            {spots}
            onselect={watchParams}
          />
        {/key}
      </div>
      <button class="splitter" type="button" aria-label={m.results_grid()} onpointerdown={drag} onkeydown={resizeKey}></button>
      <div class="results-stack">
        <ParamsBar tabId={tab.id} onrun={() => runFromParams(tab)} onleave={() => editor?.focus()} />
        <ResultsPanel {tab} />
      </div>
    </div>
    {/if}

    {#if sideHistory}
      <HistoryPanel connectionId={tab.connectionId} oninsert={(sql) => editor?.insert(sql)} onopen={(sql) => newTab(tab.connectionId, sql)} onclose={() => (workspace.historyOpen = false)} />
    {/if}
    {#if panel.open}
      <NotesPanel connectionId={tab.connectionId} dialect={namesOf(session.info) ?? sqlOf(session.info)} {editor} inQuery={tab.view === 'query' && !design} onclose={() => (panel.open = false)} />
    {/if}
  {:else if tab}
    {@const failed = workspace.openErrors[tabKey(tab)]}
    <div class="tab-waiting" role="status">
      {#if failed}
        <p class="test-result">{m.conn_open_failed()}: {failed.message}</p>
        <button class="btn" type="button" onclick={() => {
          if (!tab) return;
          delete workspace.openErrors[tabKey(tab)];
        }}>{m.org_retry()}</button>
      {:else}
        <p><span class="conn-state busy" aria-hidden="true"></span> {m.conn_connecting()}</p>
      {/if}
    </div>
  {/if}
</div>

{#if panel.saving}
  <SaveScriptDialog sql={panel.saving.sql} connectionId={panel.saving.connectionId} edit={panel.saving.edit} params={panel.saving.params} onclose={() => (panel.saving = null)} />
{/if}

<TxClose />
<ToolDialogs />
