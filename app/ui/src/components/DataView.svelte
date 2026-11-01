<script lang="ts">
  import { ArrowLeft, ArrowUpRight, ChartColumn, ChevronLeft, ChevronRight, Code, History, Link2, Lock, PanelRight, Plus, RefreshCw, SquareArrowOutUpRight, Undo2, X } from '@lucide/svelte';
  import { tick } from 'svelte';
  import { appearance } from '../lib/appearance.svelte';
  import type { FilterOp, QuickFilter } from '../lib/bindings';
  import { connections } from '../lib/connections.svelte';
  import { findTable, isBinaryType, isHex, PAGE, readOnlyReason, type TableInfo } from '../lib/data.svelte';
  import { keys } from '../lib/contextmenu.svelte';
  import { crumbLabel, fkOfColumn, forwardFilters, incoming, outgoing, targetOf } from '../lib/fk';
  import { compact, num } from '../lib/format';
  import type { GridData, GridEdit, RowView } from '../lib/grid';
  import { writesOpen } from '../lib/guard.svelte';
  import { visibilityOf } from '../lib/dbs.svelte';
  import { visibleSchema } from '../lib/visibility';
  import { nextSort } from '../lib/sort';
  import { sqlOf } from '../lib/family';
  import { qualifiedName } from '../lib/sqlname';
  import { changeDataView, discardChanges, newTab, sessionOf, type Tab } from '../lib/workspace.svelte';
  import type { ForeignKey } from '../lib/bindings';
  import { m } from '../paraglide/messages';
  import AuditPanel from './AuditPanel.svelte';
  import ChangePanel from './ChangePanel.svelte';
  import FilterAdd from './FilterAdd.svelte';
  import FkPicker from './FkPicker.svelte';
  import RelatedPanel from './RelatedPanel.svelte';
  import Grid from './Grid.svelte';
  import TablePicker from './TablePicker.svelte';
  import ValuePanel from './ValuePanel.svelte';
  import ResultChart from './ResultChart.svelte';
  import { charts, pickFrom } from '../lib/chart.svelte';
  import { registerCommands } from '../lib/palette.svelte';
  import { generateCommand } from '../lib/generate';
  import { summarizer } from '../lib/summary';
  import { toggleValue, valueCommands, valueKey, valuePane, type ValueMode } from '../lib/valuepanel.svelte';

  let { tab, session }: { tab: Tab; session: NonNullable<ReturnType<typeof sessionOf>> } = $props();

  const d = $derived(tab.data);
  const info = $derived(d.info);
  const sid = $derived(session.info.session_id);
  const production = $derived(session.info.environment === 'production');
  const open = $derived(!production || writesOpen(sid));
  const engineLocked = $derived(session.info.caps.editable ? null : session.info.server.product);
  const reason = $derived(info ? readOnlyReason(info, engineLocked) : null);
  const editable = $derived(!!info && !reason && open && !d.applying);
  const connectionName = $derived(connections.list.find((c) => c.id === tab.connectionId)?.name ?? '');
  const style = $derived(appearance.value.style);

  const PENDING = { registro: m.data_pending_registro, tavola: m.data_pending_tavola, banco: m.data_pending_banco, listino: m.data_pending_listino };
  const PAGER = { registro: m.data_page_registro, tavola: m.data_page_tavola, banco: m.data_page_banco, listino: m.data_page_listino };
  const OP: Record<FilterOp, string> = { eq: '=', ne: '≠', lt: '<', le: '≤', gt: '>', ge: '≥', contains: '∋', is_null: 'NULL', not_null: 'NOT NULL' };

  let hint = $state<string | null>(null);
  let hintTimer: ReturnType<typeof setTimeout> | undefined;
  let focused = $state<{ r: number; c: number } | null>(null);
  let where = $state('');
  let filterOpen = $state(false);
  let preset = $state<string | null>(null);

  $effect(() => {
    const column = d.filterColumn;
    if (!column || !info || d.loading) return;
    d.filterColumn = null;
    if (!info.columns.some((c) => c.name === column)) return;
    preset = column;
    filterOpen = true;
  });

  $effect(() => {
    if (!filterOpen) preset = null;
  });

  $effect.pre(() => {
    where = d.where;
  });

  $effect(() => {
    if (editable) hint = null;
  });

  function say(text: string) {
    hint = text;
    clearTimeout(hintTimer);
    hintTimer = setTimeout(() => (hint = null), 3500);
  }

  const counts = $derived((void d.rev, d.pending?.counts ?? { cells: 0, rows: 0, inserted: 0, deleted: 0, total: 0 }));
  const newCount = $derived((void d.rev, d.pending?.newRows.length ?? 0));
  const numbers = $derived((void d.rev, d.pending?.numbers() ?? new Map<string, number>()));
  const pageRows = $derived(d.page?.rows ?? []);
  const partial = $derived(!!d.page && (d.page.has_more || d.offset > 0));
  const totals = $derived(partial ? summarizer(() => session.info.session_id, () => {
    const browse = d.whole();
    if (!browse) throw new Error('no table');
    return { kind: 'table', browse };
  }) : null);
  const columns = $derived(info ? info.columns.map((c) => ({ name: c.name, type_name: c.type_name })) : []);
  const grid: GridData = $derived({
    columns,
    rows: [...Array.from({ length: newCount }, () => columns.map(() => null)), ...pageRows],
    count: newCount + pageRows.length,
    sql: info ? `SELECT * FROM ${qualifiedName(sqlOf(session.info), info.namespace, info.name, session.schema?.current_database ?? null)}` : '',
  });
  const sort = $derived(d.order.map((o) => ({ column: columns.findIndex((c) => c.name === o.column), descending: o.descending })));

  const summary = $derived(
    [
      counts.cells ? (counts.cells === 1 ? m.data_cells_one() : m.data_cells_other({ count: counts.cells })) : '',
      counts.inserted ? (counts.inserted === 1 ? m.data_new_one() : m.data_new_other({ count: counts.inserted })) : '',
      counts.deleted ? (counts.deleted === 1 ? m.data_del_one() : m.data_del_other({ count: counts.deleted })) : '',
    ]
      .filter(Boolean)
      .join(' · '),
  );

  const from = $derived(pageRows.length ? d.offset + 1 : 0);
  const to = $derived(d.offset + pageRows.length);
  const pageNo = $derived(Math.floor(d.offset / PAGE) + 1);
  const totalText = $derived(d.total !== null ? `${d.estimated ? '~' : ''}${num(d.total)}` : info?.estimated ? `~${compact(info.estimated)}` : null);

  const newAt = (r: number) => (r < newCount ? d.pending?.newRows[r] : undefined);
  const rowAt = (r: number) => (r >= newCount ? pageRows[r - newCount] : undefined);

  const edit: GridEdit = {
    cell(r, c) {
      void d.rev;
      const p = d.pending;
      if (!p) return null;
      const nr = newAt(r);
      if (nr) {
        const v = nr.values[c];
        return v === undefined ? { value: null, hint: info?.key.includes(c) ? 'auto' : m.data_default() } : { value: v };
      }
      const row = rowAt(r);
      return row ? p.cell(row, c) : null;
    },
    row(r): RowView | null {
      void d.rev;
      const p = d.pending;
      if (!p) return null;
      const nr = newAt(r);
      if (nr) {
        const mark = p.markNew(nr.id);
        return { state: 'new', n: numbers.get(`new:${nr.id}`) ?? null, mark: mark?.kind ?? null, note: mark?.kind === 'error' ? mark.message : null };
      }
      const row = rowAt(r);
      const state = row ? p.rowState(row) : null;
      if (!row || !state) return null;
      const mark = p.markRow(row);
      return {
        state,
        n: numbers.get(p.keyOf(row)) ?? null,
        mark: mark?.kind ?? null,
        note: mark ? (mark.kind === 'conflict' ? m.data_conflict_row() : mark.message) : null,
      };
    },
    applied(r, c) {
      const row = rowAt(r);
      return !!row && d.applied.size > 0 && !!d.pending && d.applied.has(`${d.pending.keyOf(row)}|${columns[c]?.name}`);
    },
    label(r) {
      return r < newCount ? m.data_new_label() : String(d.offset + r - newCount + 1);
    },
    start(r) {
      if (!editable) {
        say(reason ?? m.data_ro_locked());
        return false;
      }
      const row = rowAt(r);
      return !(row && d.pending?.rowState(row) === 'deleted');
    },
    commit(r, c, value) {
      const p = d.pending;
      if (!p || !editable) return true;
      const col = info?.columns[c];
      if (col && value !== null && isBinaryType(col.type_name) && !isHex(value)) {
        say(m.data_bad_hex({ column: col.name }));
        return false;
      }
      if (col && isBinaryType(col.type_name)) hint = null;
      const nr = newAt(r);
      if (nr) p.setNew(nr.id, c, value);
      else {
        const row = rowAt(r);
        if (row) p.set(row, c, value);
      }
      d.touch();
      return true;
    },
    setNull(r, c) {
      if (!editable) return void say(reason ?? m.data_ro_locked());
      const col = info?.columns[c];
      if (col && !col.nullable) return void say(m.data_not_null({ column: col.name }));
      void this.commit(r, c, null);
    },
    toggleDelete(r0, r1) {
      const p = d.pending;
      if (!p) return;
      if (!editable) return void say(reason ?? m.data_ro_locked());
      const ids: number[] = [];
      for (let r = r0; r <= r1; r++) {
        const nr = newAt(r);
        if (nr) ids.push(nr.id);
        else {
          const row = rowAt(r);
          if (row) p.toggleDelete(row);
        }
      }
      for (const id of ids) p.dropNew(id);
      d.touch();
    },
    focus(r, c) {
      focused = { r, c };
    },
  };

  const suggest = $derived.by(() => {
    if (!focused || !info) return null;
    const column = info.columns[focused.c]?.name;
    if (!column) return null;
    return { column, value: edit.cell(focused.r, focused.c)?.value ?? rowAt(focused.r)?.[focused.c] ?? null };
  });

  function pickTable(namespace: string, name: string) {
    const t = findTable(session.schema, namespace, name, session.info.dialect === 'oracle');
    if (t) void d.open(session.info, t);
  }

  $effect(() => {
    const wanted = tab.table;
    if (!wanted || !session.schema) return;
    tab.table = null;
    pickTable(wanted.namespace, wanted.name);
  });

  function setQuick(next: QuickFilter[]) {
    changeDataView(tab, session.info, { quick: next }, m.undo_filter());
  }

  function filterBy(c: number, value: string | null) {
    const column = columns[c]?.name;
    if (column) setQuick([...d.quick, { column, op: value === null ? 'is_null' : 'eq', value }]);
  }

  function runWhere(e: SubmitEvent) {
    e.preventDefault();
    if (where === d.where) return void d.reload(session.info);
    changeDataView(tab, session.info, { where }, m.undo_filter());
  }

  function addRow() {
    if (!editable) return say(reason ?? m.data_ro_locked());
    d.pending?.insert();
    d.touch();
  }

  const revertAll = () => discardChanges(tab);

  const fks = $derived(info ? outgoing(session.schema, info) : []);
  const hasLinks = $derived(fks.length > 0 || (!!info && incoming(session.schema, info).length > 0));
  const names = $derived(columns.map((c) => c.name));
  const fkAt = (c: number) => {
    const name = names[c];
    return name ? fkOfColumn(fks, name) : null;
  };
  const valuesAt = (r: number) =>
    names.map((_, i) => {
      const cv = edit.cell(r, i);
      return cv ? cv.value : (rowAt(r)?.[i] ?? null);
    });
  const relatedAt = $derived(focused && focused.r < grid.count ? focused.r : null);
  const relatedRow = $derived(relatedAt === null ? null : valuesAt(relatedAt));

  function targetInfo(fk: ForeignKey): TableInfo | null {
    if (!info) return null;
    const t = targetOf(info.namespace, fk);
    return findTable(session.schema, t.namespace, t.name, session.info.dialect === 'oracle');
  }

  function links(c: number) {
    const fk = fkAt(c);
    return fk ? m.fk_header({ target: `${fk.ref_table} (${fk.ref_columns.join(', ')})` }) : null;
  }

  async function go(target: TableInfo, quick: QuickFilter[], elsewhere: boolean) {
    if (!elsewhere) {
      if (await d.follow(session.info, target, quick, focused ?? undefined)) focused = null;
      return;
    }
    const t = newTab(tab.connectionId);
    t.database = tab.database;
    t.view = 'data';
    t.title = crumbLabel({ name: target.name, quick });
    void t.data.follow(session.info, target, quick);
  }

  function showRelated() {
    d.showAudit = false;
    d.showRelated = true;
    valuePane.data = false;
  }

  const side = $derived(d.showAudit ? 'audit' : d.showRelated && info ? 'related' : valuePane.data && info && d.page ? 'value' : null);
  const cell = $derived(focused && focused.r < grid.count && focused.c < columns.length ? focused : null);

  function toggleValuePanel(mode?: ValueMode) {
    if (side !== 'value') valuePane.data = false;
    if (toggleValue('data', mode)) {
      d.showAudit = false;
      d.showRelated = false;
      if (!cell) gridView?.select(0, 0);
    } else gridView?.takeFocus();
  }

  $effect(() => registerCommands(valueCommands(() => !!info && !!d.page, toggleValuePanel)));
  const chartKey = $derived(`${tab.id}:data`);
  const chartOn = $derived(!!charts.open[chartKey] && !!info);
  const toggleChart = () => {
    if (!chartOn) charts.picked[chartKey] = pickFrom(gridView?.selection(), pageRows.length, columns.length);
    charts.open[chartKey] = !chartOn;
  };
  const picked = $derived(chartOn ? charts.picked[chartKey] : null);
  $effect(() => registerCommands([{ id: 'data.chart', label: m.chart_cmd, group: m.pal_group_panels, when: () => !!info && !!d.page, run: toggleChart }]));
  $effect(() => registerCommands(generateCommand(tab, () => info)));

  function cellLock(r: number, c: number): string | null {
    if (!editable) return reason ?? m.data_ro_locked();
    const row = rowAt(r);
    if (row && d.pending?.rowState(row) === 'deleted') return m.vp_row_deleted();
    return columns[c] ? null : m.data_ro_locked();
  }

  function cellCommit(r: number, c: number, value: string | null): boolean {
    if (value !== null) return edit.commit(r, c, value);
    const col = info?.columns[c];
    if (col && !col.nullable) {
      say(m.data_not_null({ column: col.name }));
      return false;
    }
    return edit.commit(r, c, null);
  }

  const cellAt = (r: number, c: number) => {
    const cv = edit.cell(r, c);
    return cv ? cv.value : (rowAt(r)?.[c] ?? null);
  };

  function follow(r: number, c: number, elsewhere: boolean) {
    const fk = fkAt(c);
    const target = fk && targetInfo(fk);
    if (!fk || !target) return showRelated();
    const quick = forwardFilters(fk, names, valuesAt(r));
    if (!quick) return say(m.fk_null_cell({ column: fk.columns.join(', ') }));
    void go(target, quick, elsewhere);
  }

  function linkMenu(r: number, c: number) {
    if (!hasLinks) return [];
    const fk = fkAt(c);
    return [
      !!fk && { label: m.fk_follow(), icon: ArrowUpRight, shortcut: 'F4', run: () => follow(r, c, false) },
      !!fk && { label: m.fk_follow_tab(), icon: SquareArrowOutUpRight, shortcut: keys('Shift+F4'), run: () => follow(r, c, true) },
      { label: m.fk_related_show(), icon: Link2, ...(fk ? {} : { shortcut: 'F4' }), run: showRelated },
    ];
  }

  function back(at: number) {
    const crumb = d.trail[at];
    const t = crumb && findTable(session.schema, crumb.namespace, crumb.name, session.info.dialect === 'oracle');
    if (t) {
      focused = null;
      void d.back(session.info, t, at).then(async (f) => {
        if (!f) return;
        await tick();
        gridView?.select(f.r, f.c);
      });
    }
  }


  function forward() {
    const crumb = d.ahead[d.ahead.length - 1];
    const t = crumb && findTable(session.schema, crumb.namespace, crumb.name, session.info.dialect === 'oracle');
    if (t) {
      focused = null;
      void d.forward(session.info, t);
    }
  }

  let root = $state<HTMLElement>();

  function onwindowkey(e: KeyboardEvent) {
    if (!e.altKey || e.ctrlKey || e.metaKey || e.defaultPrevented || !root?.checkVisibility()) return;
    if (e.target instanceof HTMLInputElement || e.target instanceof HTMLTextAreaElement || (e.target as HTMLElement | null)?.isContentEditable) return;
    if (e.key === 'ArrowLeft' && d.trail.length) back(d.trail.length - 1);
    else if (e.key === 'ArrowRight' && d.ahead.length) forward();
    else return;
    e.preventDefault();
  }

  const sideButton = (e: MouseEvent) => (e.button === 3 || e.button === 4) && e.target instanceof Node && !!root?.contains(e.target);

  function onmouseup(e: MouseEvent) {
    if (!sideButton(e)) return;
    e.preventDefault();
    if (e.button === 3 && d.trail.length) back(d.trail.length - 1);
    else if (e.button === 4) forward();
  }

  type Picking = { r: number; c: number; fk: ForeignKey; target: TableInfo; column: string; text: string; original: string | null; anchor: DOMRect };
  let picking = $state.raw<Picking | null>(null);
  let picker = $state<ReturnType<typeof FkPicker>>();
  let gridView = $state<ReturnType<typeof Grid>>();

  function onedit(e: { r: number; c: number; value: string; input: HTMLInputElement } | null) {
    const fk = e && fkAt(e.c);
    const target = fk && targetInfo(fk);
    const name = e && names[e.c];
    const column = fk && name ? fk.ref_columns[fk.columns.indexOf(name)] : undefined;
    if (!e || !fk || !target || !column) {
      picking = null;
      return;
    }
    picking = { r: e.r, c: e.c, fk, target, column, text: e.value, original: valuesAt(e.r)[e.c] ?? null, anchor: e.input.getBoundingClientRect() };
  }

  function pick(values: Map<string, string | null>, step: boolean) {
    const p = picking;
    if (!p) return;
    for (const [column, value] of values) {
      const i = names.indexOf(column);
      if (i >= 0 && i !== p.c) edit.commit(p.r, i, value);
    }
    gridView?.setEditValue(values.get(names[p.c] ?? '') ?? '');
    gridView?.commitEdit(step ? [1, 0] : undefined);
  }

  function onkeydown(e: KeyboardEvent) {
    if (e.key === 'F4' && focused && hasLinks && !e.defaultPrevented && !(e.target instanceof HTMLInputElement)) {
      e.preventDefault();
      follow(focused.r, focused.c, e.shiftKey);
      return;
    }
    if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === 's') {
      e.preventDefault();
      void d.apply(session.info, connectionName);
    }
  }
</script>

<svelte:window onkeydown={onwindowkey} {onmouseup} onmousedown={(e) => sideButton(e) && e.preventDefault()} />
<!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
<section bind:this={root} class="dv" class:with-audit={side === 'audit'} class:with-related={side === 'related'} class:with-value={side === 'value'} class:with-sql={d.showSql && counts.total > 0} class:pending={counts.total > 0} aria-label={m.view_data()} {onkeydown}>
  <header class="dv-head">
    {#if d.trail.length}
      {@const last = d.trail[d.trail.length - 1]}
      <nav class="fk-trail" aria-label={m.fk_trail()}>
        <button class="icon-btn fk-back" type="button" aria-label={m.fk_back({ table: last?.name ?? '' })} data-tip={`${m.fk_back({ table: last?.name ?? '' })} (${keys('Alt')}+←)`} onclick={() => back(d.trail.length - 1)}><ArrowLeft size={14} strokeWidth={1.8} /></button>
        <ol class="fk-crumbs">
          {#if d.trail.length > 2}<li class="fk-more" data-tip={d.trail.slice(0, -2).map(crumbLabel).join(' › ')}>…<ChevronRight size={12} strokeWidth={1.8} aria-hidden="true" /></li>{/if}
          {#each d.trail as c, i (i)}
            {#if i >= d.trail.length - 2}
              <li><button class="fk-crumb" type="button" data-tip={crumbLabel(c)} onclick={() => back(i)}>{c.name}</button><ChevronRight size={12} strokeWidth={1.8} aria-hidden="true" /></li>
            {/if}
          {/each}
        </ol>
      </nav>
    {/if}
    <TablePicker schema={session.schema && visibleSchema(session.schema, visibilityOf(tab.connectionId))} current={info} onpick={pickTable} />
    {#if info}
      <span class="dv-meta">
        {#if totalText}{d.total === 1 ? m.fk_rows_one() : d.total !== null && d.estimated ? m.data_rows_estimate({ count: num(d.total) }) : d.total !== null ? m.data_rows_exact({ count: totalText }) : m.data_rows_estimate({ count: compact(info.estimated ?? 0) })}{/if}
        {#if info.namespace}<span>· {info.namespace}</span>{/if}
        {#if info.key.length}<span>· {m.data_key({ key: info.key.map((i) => info.columns[i]?.name).join(', ') })}</span>{/if}
      </span>
    {/if}
    <span class="spacer"></span>
    {#if info}
      {#if reason}
        <span class="dv-ro" data-tip={engineLocked ? m.data_ro_engine_hint() : undefined}><Lock size={12} strokeWidth={1.8} aria-hidden="true" />{reason}</span>
      {:else}
        <button class="btn" type="button" onclick={addRow} disabled={!open}><Plus size={13} strokeWidth={1.8} aria-hidden="true" />{m.data_new_row()}</button>
      {/if}
    {/if}
    {#if info && hasLinks}
      <button class="btn ghost" type="button" aria-pressed={d.showRelated && !d.showAudit} onclick={() => (d.showRelated && !d.showAudit ? (d.showRelated = false) : showRelated())}>
        <Link2 size={13} strokeWidth={1.8} aria-hidden="true" />
        {m.fk_related()}
      </button>
    {/if}
    {#if info}
      <button class="btn ghost dv-chart" type="button" aria-pressed={chartOn} aria-label={m.chart_tab()} data-tip={m.chart_tab()} disabled={!d.page} onclick={toggleChart}>
        <ChartColumn size={14} strokeWidth={1.8} aria-hidden="true" />
      </button>
      <button class="btn ghost" type="button" aria-pressed={side === 'value'} data-tip={keys('Shift+Enter')} disabled={!d.page} onclick={() => toggleValuePanel('value')}>
        <PanelRight size={13} strokeWidth={1.8} aria-hidden="true" />
        {m.vp_button()}
      </button>
    {/if}
    <button class="btn ghost" type="button" aria-pressed={d.showAudit} onclick={() => ((d.showAudit = !d.showAudit), d.showAudit && (valuePane.data = false))}>
      <History size={13} strokeWidth={1.8} aria-hidden="true" />
      {m.data_audit()}
    </button>
  </header>

  {#if info}
    <div class="dv-filters">
      <form class="field dv-where" onsubmit={runWhere}>
        <span class="dv-where-kw" aria-hidden="true">WHERE</span>
        <input bind:value={where} placeholder={m.data_where()} aria-label={m.data_where_label()} spellcheck="false" />
      </form>
      {#each d.quick as f, i (i)}
        <span class="dv-chip">
          <code>{f.column} {OP[f.op]}{f.value !== null && f.value !== undefined ? ` ${f.value}` : ''}</code>
          <button type="button" aria-label={m.data_filter_remove()} onclick={() => setQuick(d.quick.filter((_, k) => k !== i))}><X size={11} strokeWidth={2} /></button>
        </span>
      {/each}
      <FilterAdd {columns} suggest={preset ? { column: preset, value: '' } : suggest} onadd={(f) => setQuick([...d.quick, f])} bind:open={filterOpen} />
      <span class="spacer"></span>
      <nav class="dv-pager" aria-label={m.data_next()}>
        <span class="dv-range">{to ? (totalText ? m.data_range({ from: num(from), to: num(to), total: totalText }) : m.data_range_open({ from: num(from), to: num(to) })) : ''}</span>
        <button class="icon-btn" type="button" aria-label={m.data_prev()} disabled={d.offset === 0 || d.loading} onclick={() => void d.reload(session.info, Math.max(0, d.offset - PAGE))}><ChevronLeft size={15} strokeWidth={1.8} /></button>
        <span class="dv-page">{PAGER[style]({ page: num(pageNo), pages: d.pages !== null ? num(d.pages) : '…' })}</span>
        <button class="icon-btn" type="button" aria-label={m.data_next()} disabled={!d.page?.has_more || d.loading} onclick={() => void d.reload(session.info, d.offset + PAGE)}><ChevronRight size={15} strokeWidth={1.8} /></button>
        <button class="icon-btn" type="button" aria-label={m.data_reload()} disabled={d.loading} onclick={() => void d.load(session.info)}><RefreshCw size={13} strokeWidth={1.8} /></button>
      </nav>
    </div>
  {/if}

  <div class="dv-body" class:loading={d.loading}>
    {#if !info}
      <p class="dv-empty">{m.data_empty()}</p>
    {:else if d.error}
      <p class="results-error dv-error">{d.error}</p>
    {:else if d.page && chartOn && info}
      {#if picked}<ResultChart columns={picked.columns} rows={picked.rows} rev={0} key={`${chartKey}:picked`} name={info.name} />
      {:else}<ResultChart {columns} rows={pageRows} rev={d.offset} key={chartKey} name={info.name} partial={partial ? pageRows.length : 0} groups={totals?.groups ?? null} />{/if}
    {:else if d.page}
      <Grid bind:this={gridView} result={grid} dialect={sqlOf(session.info)} {partial} summarize={totals?.stats ?? null} {edit} {sort} links={hasLinks ? links : undefined} onfollow={hasLinks ? follow : undefined} cellmenu={linkMenu} {onedit} editkey={(e) => picker?.key(e) ?? false} gridkey={(e) => valueKey(e, toggleValuePanel)} onsort={(c, add, dir) => changeDataView(tab, session.info, { order: nextSort(d.order, columns[c]?.name ?? '', add, dir) }, m.undo_sorted())} onfilter={filterBy} />
      {#if grid.count === 0}<p class="dv-none">{m.data_no_rows()}</p>{/if}
    {:else}
      <p class="dv-empty">{m.data_loading()}</p>
    {/if}
  </div>

  {#if side === 'value'}
    <ValuePanel {columns} count={grid.count} focus={cell} at={cellAt} rowLabel={(r) => edit.label(r)} dialect={session.info.dialect ?? null} mode={valuePane.mode} lock={cellLock} commit={cellCommit} onmove={(r, c) => gridView?.select(r, c)} onmode={(mode) => (valuePane.mode = mode)} onclose={() => toggleValuePanel(valuePane.mode)} />
  {:else if d.showAudit}
    <AuditPanel connectionId={tab.connectionId} rev={d.auditRev} onclose={() => (d.showAudit = false)} />
  {:else if d.showRelated && info}
    <RelatedPanel session={session.info} schema={session.schema} {info} row={relatedRow} label={relatedAt === null ? null : m.fk_row({ n: edit.label(relatedAt) })} onfollow={(t, q, elsewhere) => void go(t, q, elsewhere)} onclose={() => (d.showRelated = false)} />
  {/if}
  {#if picking}
    <FkPicker bind:this={picker} session={session.info} target={picking.target} fk={picking.fk} column={picking.column} text={picking.text} original={picking.original} anchor={picking.anchor} onpick={pick} />
  {/if}

  {#if info && !engineLocked}
    <footer class="dv-bar">
      <span class="dv-lamp" aria-hidden="true"></span>
      {#if counts.total > 0}
        <span class="dv-pending-title">{PENDING[style]()}</span>
        <span class="dv-summary">{summary}</span>
      {:else}
        <span class="dv-summary idle">{m.data_no_changes()}</span>
      {/if}
      {#if hint}
        <span class="dv-msg">{hint}</span>
      {:else if d.message}
        <span class="dv-msg {d.message.kind}" role="status">{d.message.text}</span>
      {:else if editable && counts.total === 0}
        <span class="dv-keys">{m.data_keys_hint()}{#if fks.length} · {m.fk_keys_hint()}{/if}</span>
      {/if}
      <span class="spacer"></span>
      {#if counts.total > 0}
        <button class="btn ghost" type="button" onclick={revertAll}><Undo2 size={13} strokeWidth={1.8} aria-hidden="true" />{m.data_revert_all()}</button>
        <button class="btn ghost" type="button" aria-pressed={d.showSql} onclick={() => (d.showSql = !d.showSql)}>
          <Code size={13} strokeWidth={1.8} aria-hidden="true" />
          {d.showSql ? m.data_hide_sql() : m.data_show_sql()}
        </button>
      {/if}
      <button class="btn primary dv-apply" type="button" disabled={counts.total === 0 || d.applying || !open} data-tip={open ? '' : m.data_ro_locked()} onclick={() => void d.apply(session.info, connectionName)}>
        {d.applying ? m.data_applying() : m.data_apply()}
        {#if counts.total > 0}<span class="dv-count">{counts.total}</span>{/if}
      </button>
    </footer>
    {#if d.showSql && counts.total > 0}
      <ChangePanel {d} session={session.info} />
    {/if}
  {/if}
</section>
