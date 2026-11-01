<script lang="ts">
  import { tabKey } from '../lib/tabs';
  import { ICON, TOOL_ICON } from '../lib/icons';
  import { ChartColumn, Check, ClipboardCopy, FileDown, FlaskConical, Info, PanelRight, RotateCcw, Undo2, Unplug, X } from '@lucide/svelte';
  import { keys, openMenu } from '../lib/contextmenu.svelte';
  import { askExport, compareResult } from '../lib/tools.svelte';
  import { untrack } from 'svelte';
  import { driverMessage } from '../lib/api';
  import type { CopyFormat } from '../lib/copy';
  import { num } from '../lib/format';
  import { planFor } from '../lib/plan.svelte';
  import { nextSort, type SortDir } from '../lib/sort';
  import { droppedNotice } from '../lib/session';
  import { loadAll, rerunDropped, revive, sessionOf, sortResult, type ResultSet, type Tab } from '../lib/workspace.svelte';
  import { m } from '../paraglide/messages';
  import Grid from './Grid.svelte';
  import Menu from './Menu.svelte';
  import PlanView from './PlanView.svelte';
  import DryRunView from './DryRunView.svelte';
  import { txSummary } from '../lib/tx';
  import { commitTx, rollbackTx, txOf, txs } from '../lib/tx.svelte';
  import { registerCommands } from '../lib/palette.svelte';
  import { toggleValue, valueCommands, valueKey, valuePane, type ValueMode } from '../lib/valuepanel.svelte';
  import ValuePanel from './ValuePanel.svelte';
  import ResultChart from './ResultChart.svelte';
  import { charts, pickFrom } from '../lib/chart.svelte';
  import { isSql, namesOf } from '../lib/family';
  import DocView from './DocView.svelte';
  import Segmented from './Segmented.svelte';
  import RunProgress from './RunProgress.svelte';
  import { summarizer } from '../lib/summary';

  let { tab }: { tab: Tab } = $props();
  let grid = $state<Grid | null>(null);
  let showMessages = $state(false);
  let showPlan = $state(false);
  let showDry = $state(false);
  const plan = $derived(planFor(tab.id));
  const chartKey = $derived(`${tab.id}:results`);
  const chartOn = $derived(!!charts.open[chartKey] && !showMessages && !showPlan && !showDry);
  const showChart = (on: boolean) => {
    if (on && !charts.open[chartKey]) charts.picked[chartKey] = pickFrom(grid?.selection(), current?.rows.length ?? 0, current?.columns.length ?? 0);
    charts.open[chartKey] = on;
  };
  const picked = $derived(chartOn ? charts.picked[chartKey] : null);
  let blocked = $state<string | null>(null);
  let blockedTimer: ReturnType<typeof setTimeout> | undefined;

  const grids = $derived(tab.results.filter((r) => r.hasGrid));
  const current = $derived(tab.results[tab.activeResult]);
  const failed = $derived(tab.results.find((r) => r.error));
  const announce = $derived.by(() => {
    const f = tab.finished;
    if (!f || tab.running || tab.failure) return '';
    if (f.cancelled) return m.results_cancelled();
    const bad = tab.results.filter((r) => r.error).length;
    if (bad) return m.results_done_failed({ failed: bad, count: tab.results.length });
    const ms = fmt.format(f.elapsed);
    return current?.hasGrid ? `${m.results_rows({ count: fmt.format(current.count) })} · ${ms} ms` : m.results_done_ok({ ms });
  });
  const fmt = { format: num };
  const dialect = $derived.by(() => {
    const s = sessionOf(tab);
    return s ? namesOf(s.info) : null;
  });
  const caps = $derived(sessionOf(tab)?.info.caps ?? null);
  const sql = $derived(!caps || isSql(caps));
  const partial = $derived(!!current?.done && (!!current.done.rowLimit || !!current.done.truncated));
  const summary = $derived(current && sql && !current.unsortable && partial ? summarizer(() => sessionOf(tab)?.info.session_id ?? null, () => ({ kind: 'query', sql: current.sql })) : null);
  const treeShown = $derived(!!current?.isDocs && current.docMode === 'tree');
  const DOC_MODES = [
    { value: 'tree' as const, label: m.doc_mode_tree() },
    { value: 'table' as const, label: m.doc_mode_table() },
  ];
  const sid = $derived(sessionOf(tab)?.info.session_id ?? null);
  const tx = $derived(txOf(sid));
  const dryWrites = $derived(tab.dry && tab.results.some((r) => r.preview || (r.done?.rowsAffected ?? null) !== null));
  const dryState = $derived(!tab.dry || !tab.finished ? null : tx?.dry_run ? 'open' : failed || tab.failure ? 'failed' : sid ? (txs.last[sid] ?? null) : null);
  const DRY_DONE = { committed: m.tx_dry_committed, rolled_back: m.tx_dry_rolled_back, failed: m.tx_dry_failed };

  const WHY = { not_select: m.sort_not_select, writes: m.sort_writes, multiple: m.sort_multiple };

  function whyNot(r: ResultSet): string | null {
    if (caps && !caps.sort) return m.sort_not_supported();
    if (r.unsortable) return WHY[r.unsortable]();
    if (dialect === 'mysql' && new Set(r.columns.map((c) => c.name)).size < r.columns.length) return m.sort_duplicates();
    return null;
  }

  const reason = $derived(current ? whyNot(current) : null);

  function sortBy(c: number, add: boolean, dir?: SortDir) {
    if (!current || reason) return;
    if (!current.done) return flash(m.sort_running());
    void sortResult(tab, current, nextSort(current.sort, c, add, dir));
  }

  function flash(text = reason) {
    blocked = text;
    clearTimeout(blockedTimer);
    blockedTimer = setTimeout(() => (blocked = null), 4000);
  }

  $effect(() => {
    void plan?.seq;
    untrack(() => (showPlan = !!plan));
  });

  $effect(() => {
    const finished = tab.finished;
    untrack(() => {
      showPlan = false;
      showDry = !!finished && dryWrites && !failed;
      if (!finished) showMessages = false;
      else if (failed || (grids.length === 0 && !showDry)) showMessages = true;
    });
  });

  function outcome(r: (typeof tab.results)[number]): string {
    if (r.error) return driverMessage(r.error.error);
    if (!r.done) return m.results_running();
    const parts = [r.done.command ?? m.results_ok()];
    if (r.done.rowsAffected !== null) parts.push(m.results_affected({ count: fmt.format(r.done.rowsAffected) }));
    else if (r.hasGrid) parts.push(m.results_rows({ count: fmt.format(r.count) }));
    parts.push(`${fmt.format(r.done.elapsed)} ms`);
    return parts.join(' · ');
  }

  const comparable = (r: ResultSet) => !!r.done && !r.unsortable;

  let cell = $state<{ r: number; c: number } | null>(null);
  const gridShown = $derived(!!current?.hasGrid && !treeShown && !chartOn && !showMessages && !showPlan && !showDry && !tab.failure && current.count > 0);
  const withValue = $derived(valuePane.results && gridShown);

  function toggleValuePanel(mode?: ValueMode) {
    if (toggleValue('results', mode) && !cell) grid?.select(0, 0);
    else if (!valuePane.results) grid?.takeFocus();
  }

  $effect(() => registerCommands(valueCommands(() => gridShown, toggleValuePanel)));
  $effect(() => registerCommands([{ id: 'results.doc_mode', label: () => (current?.docMode === 'table' ? m.pal_cmd_doc_tree() : m.pal_cmd_doc_table()), group: m.pal_group_panels, when: () => !!current?.isDocs, run: () => current && (current.docMode = current.docMode === 'table' ? 'tree' : 'table') }]));
  $effect(() => registerCommands([{ id: 'results.chart', label: m.chart_cmd, group: m.pal_group_panels, when: () => !!current?.hasGrid, run: () => (showChart(!chartOn), (showMessages = false), (showPlan = false), (showDry = false)) }]));

  $effect(() =>
    registerCommands([
      {
        id: 'results.copy_as',
        label: m.pal_cmd_copy_result_as,
        group: m.pal_group_panels,
        icon: ICON.copy,
        when: () => gridShown,
        run: () => {},
        ask: { prompt: m.pal_ask_format, choices: () => formats.map((f) => ({ id: f.value, label: f.label, icon: ICON.copy })), run: (f) => grid?.copy(f as CopyFormat) },
      },
      { id: 'results.export', label: m.pal_cmd_export_result, group: m.pal_group_panels, icon: ICON.export, when: () => !!current && comparable(current) && sql, run: () => current && askExport({ connectionId: tab.connectionId, source: { kind: 'query', sql: current.sql }, label: m.tool_export_result_file(), expected: null }) },
      { id: 'results.compare', label: m.pal_cmd_compare_result, group: m.pal_group_panels, icon: TOOL_ICON.rows, when: () => !!current && comparable(current) && sql, run: () => current && compareResult(tab, current.sql) },
    ]),
  );

  function resultMenu(e: MouseEvent, r: ResultSet) {
    e.preventDefault();
    openMenu(e, [
      { label: m.rows_compare_with(), icon: TOOL_ICON.rows, disabled: !comparable(r) || !sql, run: () => compareResult(tab, r.sql) },
      { label: m.tool_export_short(), icon: ICON.export, disabled: !comparable(r) || !sql, run: () => askExport({ connectionId: tab.connectionId, source: { kind: 'query', sql: r.sql }, label: m.tool_export_result_file(), expected: null }) },
    ]);
  }

  const formats: { value: CopyFormat; label: string }[] = [
    { value: 'tsv', label: m.copy_tsv() },
    { value: 'csv', label: m.copy_csv() },
    { value: 'json', label: m.copy_json() },
    { value: 'insert', label: m.copy_insert() },
  ];
  const uid = $props.id();
  const selected = $derived(
    showPlan && plan ? 'plan' : showDry && dryWrites ? 'dry' : showMessages ? 'messages' : chartOn && current?.hasGrid ? 'chart' : grids.some((r) => r.index === tab.activeResult) ? `grid-${tab.activeResult}` : 'messages',
  );
  const tabAttrs = (id: string) => ({ id: `${uid}-${id}`, 'aria-controls': `${uid}-panel`, tabindex: selected === id ? 0 : -1 });
</script>

<section class="results" aria-label={m.results_grid()}>
  <p class="sr-only" role="status" aria-atomic="true">{announce}</p>
  <div class="results-bar">
    <div class="results-tabs" role="tablist" aria-label={m.results_grid()} tabindex="-1" onkeydown={tabKey}>
      {#each grids as r (r.index)}
        <button
          type="button"
          role="tab"
          {...tabAttrs(`grid-${r.index}`)}
          aria-selected={!showPlan && !showDry && !showMessages && !chartOn && tab.activeResult === r.index}
          oncontextmenu={(e) => resultMenu(e, r)}
          onclick={() => {
            tab.activeResult = r.index;
            showChart(false);
            showMessages = false;
            showPlan = false;
            showDry = false;
          }}>{m.results_grid()} {grids.length > 1 ? r.index + 1 : ''}</button
        >
      {/each}
      {#if current?.hasGrid}<button type="button" role="tab" {...tabAttrs('chart')} class="chart-tab" aria-selected={chartOn} aria-label={m.chart_tab()} data-tip={m.chart_tab()} onclick={() => (showChart(true), (showMessages = false), (showPlan = false), (showDry = false))}><ChartColumn size={13} strokeWidth={1.8} aria-hidden="true" /><span class="chart-tab-label">{m.chart_tab()}</span></button>{/if}
      {#if dryWrites}<button type="button" role="tab" {...tabAttrs('dry')} class="dry-tab" aria-selected={showDry && !showPlan} onclick={() => ((showDry = true), (showPlan = false), (showMessages = false))}><FlaskConical size={13} strokeWidth={1.8} aria-hidden="true" />{m.tx_dry_tab()}</button>{/if}
      <button type="button" role="tab" {...tabAttrs('messages')} aria-selected={!showPlan && !showDry && showMessages} class:has-error={!!failed} onclick={() => ((showMessages = true), (showPlan = false), (showDry = false))}>{m.results_messages()}</button>
      {#if plan}<button type="button" role="tab" {...tabAttrs('plan')} aria-selected={showPlan} onclick={() => (showPlan = true)}>{m.plan_tab()}</button>{/if}
    </div>
    <span class="spacer"></span>
    {#if current?.hasGrid && chartOn && picked}
      <span class="results-meta" data-testid="chart-picked">{m.chart_picked({ rows: fmt.format(picked.rows.length), columns: picked.columns.length })}</span>
      <button class="btn ghost" type="button" onclick={() => (charts.picked[chartKey] = null)}>{m.chart_picked_all()}</button>
    {:else if current?.hasGrid && chartOn}
      <span class="results-meta">{m.results_rows({ count: fmt.format(current.count) })}</span>
    {:else if current?.hasGrid && !showMessages && !showPlan && !showDry}
      {#if blocked}<span class="results-sortnote" role="status">{blocked}</span>{/if}
      {#if current.isDocs}{@const r = current}<Segmented class="doc-mode" label={m.doc_mode()} value={r.docMode} options={DOC_MODES} onchange={(v) => (r.docMode = v)} />{/if}
      <span class="results-meta">
        {m.results_rows({ count: fmt.format(current.count) })}{#if current.sorting}&nbsp;· {m.results_sorting()}{:else if current.done}&nbsp;· {fmt.format(current.done.elapsed)} ms{:else}&nbsp;· {m.results_running()}{/if}
      </span>
      {#if !treeShown}
      <button class="btn ghost" type="button" aria-pressed={valuePane.results} data-tip={keys('Shift+Enter')} disabled={current.count === 0} onclick={() => toggleValuePanel('value')}>
        <PanelRight size={14} strokeWidth={1.8} aria-hidden="true" />
        {m.vp_button()}
      </button>
      <Menu label={m.copy()} items={formats} onpick={(f) => void grid?.copy(f)} disabled={current.count === 0}>
        <ClipboardCopy size={14} strokeWidth={1.8} aria-hidden="true" />
        {m.copy()}
      </Menu>
      {/if}
      {#if sql}
      <button class="btn ghost" type="button" disabled={!current.done || !!current.unsortable} data-tip={current.unsortable ? m.tool_export_not_read() : undefined} onclick={() => askExport({ connectionId: tab.connectionId, source: { kind: 'query', sql: current.sql }, label: m.tool_export_result_file(), expected: null })}>
        <FileDown size={14} strokeWidth={1.8} aria-hidden="true" />
        {m.tool_export_short()}
      </button>
      {/if}
    {/if}
  </div>

  <div class="results-notes">
  <RunProgress {tab} />
  {#if dryState === 'open' && tx && sid}
    <div class="dry-banner" role="status">
      <FlaskConical size={14} strokeWidth={1.8} aria-hidden="true" />
      <p>{m.tx_dry_banner({ summary: txSummary(tx) })}</p>
      <button class="btn" type="button" disabled={!!txs.busy[sid]} onclick={() => void rollbackTx(sid)}><Undo2 size={13} strokeWidth={1.8} aria-hidden="true" /> {m.tx_rollback()}</button>
      <button class="btn primary" type="button" disabled={!!txs.busy[sid]} onclick={() => void commitTx(sid)}><Check size={13} strokeWidth={2} aria-hidden="true" /> {m.tx_commit()}</button>
    </div>
  {:else if dryState && dryState !== 'open'}
    <div class="dry-banner done" class:failed={dryState === 'failed'} role="status">
      <FlaskConical size={14} strokeWidth={1.8} aria-hidden="true" />
      <p>{DRY_DONE[dryState]()}</p>
    </div>
  {/if}
  {#if tab.notice}
    <div class="session-notice tx-notice" role="alert">
      <Info size={14} strokeWidth={1.8} aria-hidden="true" />
      <p>{tab.notice}</p>
      <button class="icon-btn" type="button" aria-label={m.session_dismiss()} onclick={() => (tab.notice = null)}><X size={13} strokeWidth={2} /></button>
    </div>
  {/if}
  {#if tab.dropped}
    {@const d = tab.dropped}
    <div class="session-notice" class:write={d.write} class:lost={!d.restored} role="alert">
      <Unplug size={14} strokeWidth={1.8} aria-hidden="true" />
      <p>{droppedNotice(d)}{#if d.error}<span class="session-notice-err">{d.error}</span>{/if}</p>
      {#if d.restored}
        <button class="btn" type="button" disabled={tab.running} onclick={() => rerunDropped(tab)}><RotateCcw size={13} strokeWidth={1.8} aria-hidden="true" /> {d.write ? m.session_rerun_anyway() : m.session_rerun()}</button>
      {:else}
        {@const s = sessionOf(tab)}
        <button class="btn" type="button" onclick={() => s && void revive(s).then((ok) => ok && tab.dropped && (tab.dropped = { ...tab.dropped, restored: true, error: null }))}>{m.session_reconnect_now()}</button>
      {/if}
      <button class="icon-btn" type="button" aria-label={m.session_dismiss()} onclick={() => (tab.dropped = null)}><X size={13} strokeWidth={2} /></button>
    </div>
  {/if}

  {#if current?.sortError && !showMessages && !showPlan && !showDry}
    <p class="results-note err" role="alert">{m.sort_failed({ message: current.sortError })}</p>
  {:else if current?.done?.rowLimit && !showMessages && !showPlan && !showDry}
    {@const r = current}
    <p class="results-note limit">
      <span>{m.results_limited({ count: fmt.format(r.done?.rowLimit ?? r.count) })}</span>
      <button class="btn ghost" type="button" disabled={tab.running} onclick={() => void loadAll(tab, r)}>{r.sorting ? m.results_loading_all() : m.results_load_all()}</button>
    </p>
  {:else if current?.done?.truncated && !showMessages && !showPlan && !showDry}
    <p class="results-note">{m.results_truncated({ count: fmt.format(current.count) })}</p>
  {/if}
  </div>

  <div class="results-body" class:with-value={withValue} role="tabpanel" id="{uid}-panel" aria-labelledby="{uid}-{selected}">
    {#if showPlan && plan}
      <PlanView {tab} />
    {:else if showDry && dryWrites}
      <DryRunView {tab} />
    {:else if tab.failure}
      <p class="results-error" role="alert">{tab.failure}</p>
    {:else if tab.results.length === 0}
      <p class="results-empty">{tab.running ? m.results_running() : m.results_empty()}</p>
    {:else if showMessages || !current?.hasGrid}
      <ol class="messages">
        {#each tab.results as r (r.index)}
          <li class:err={!!r.error}>
            <code>{r.sql.length > 160 ? `${r.sql.slice(0, 160)}…` : r.sql}</code>
            <span class="msg">{outcome(r)}</span>
            {#if r.error?.position}<span class="msg-at">{m.results_at({ line: r.error.position.line, column: r.error.position.column })}</span>{/if}
            {#if r.error?.error.kind === 'query' && r.error.error.detail}<span class="msg-extra">{r.error.error.detail}</span>{/if}
            {#if r.error?.error.kind === 'query' && r.error.error.hint}<span class="msg-extra">{m.results_hint()}: {r.error.error.hint}</span>{/if}
          </li>
        {/each}
        {#if tab.finished?.cancelled}<li class="err"><span class="msg">{m.results_cancelled()}</span></li>{/if}
      </ol>
    {:else if chartOn}
      {@const r = current}
      {#if picked}<ResultChart columns={picked.columns} rows={picked.rows} rev={0} key={`${chartKey}:picked`} name={tab.title || m.chart_tab()} />
      {:else}<ResultChart columns={r.columns} rows={r.rows} rev={r.rev + r.count} key={chartKey} name={tab.title || m.chart_tab()} partial={partial ? r.count : 0} groups={summary?.groups ?? null} />{/if}
    {:else if current.count === 0 && current.done}
      <p class="results-empty">{m.results_no_rows()}</p>
    {:else if treeShown}
      {#key current}<DocView result={current} {tab} />{/key}
    {:else}
      {#key current}
        <Grid bind:this={grid} result={current} {dialect} {partial} summarize={summary?.stats ?? null} sort={current.sort} onsort={sortBy} unsortable={reason} onblocked={() => flash()} onfocus={(f) => (cell = f)} gridkey={(e) => valueKey(e, toggleValuePanel)} extra={[{ label: m.rows_compare_with(), icon: TOOL_ICON.rows, disabled: !comparable(current) || !sql, run: () => compareResult(tab, current.sql) }]} />
      {/key}
      {#if withValue}
        {@const r = current}
        <ValuePanel columns={r.columns} count={r.count} focus={cell} at={(row, c) => (void r.rev, void r.count, r.rows[row]?.[c] ?? null)} rowLabel={(row) => String(row + 1)} dialect={dialect ?? null} mode={valuePane.mode} onmove={(row, c) => grid?.select(row, c)} onmode={(mode) => (valuePane.mode = mode)} onclose={() => toggleValuePanel(valuePane.mode)} />
      {/if}
    {/if}
  </div>
</section>
