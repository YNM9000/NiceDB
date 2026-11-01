<script lang="ts">
  import { erFile } from '../lib/file-names';
  import { sqlOf } from '../lib/family';
  import { ClipboardCopy, Download, FileCode, LayoutGrid, Maximize, Minus, Network, PencilRuler, Plus, Search, Table2, TextCursorInput } from '@lucide/svelte';
  import { copyText, openMenu } from '../lib/contextmenu.svelte';
  import { copyDdl, showDdl } from '../lib/ddl.svelte';
  import { openDesigner } from '../lib/design.svelte';
  import { subjectOf } from '../lib/sqlgen';
  import { sqlEntries } from '../lib/sqlgen.svelte';
  import { selectSql } from '../lib/sqlname';
  import { onMount } from 'svelte';
  import { api } from '../lib/api';
  import { autoLayout } from '../lib/er/elk';
  import { toPng, toSvg } from '../lib/er/export';
  import { applySaved, toSaved } from '../lib/er/layout';
  import { areaLabels, buildGraph, neighbours, routeAll, search, type ErGraph, type ErTable } from '../lib/er/model';
  import { readLook, scene, type Scene } from '../lib/er/render';
  import { Stage } from '../lib/er/stage';
  import type { Tab } from '../lib/workspace.svelte';
  import { visibilityOf } from '../lib/dbs.svelte';
  import { visibleSchema } from '../lib/visibility';
  import { openTable, queryTab, sessionOf } from '../lib/workspace.svelte';
  import { m } from '../paraglide/messages';
  import { ICON } from '../lib/icons';
  import { registerCommands } from '../lib/palette.svelte';
  import ErFilter from './ErFilter.svelte';
  import ErLegend from './ErLegend.svelte';
  import ErPanel from './ErPanel.svelte';
  import Menu from './Menu.svelte';

  let { tab, session }: { tab: Tab; session: NonNullable<ReturnType<typeof sessionOf>> } = $props();

  let host = $state<HTMLDivElement>();
  let canvas = $state<HTMLCanvasElement>();
  let mini = $state<HTMLCanvasElement>();
  let stage: Stage | null = null;

  let graph = $state.raw<ErGraph | null>(null);
  let current = $state.raw<Scene | null>(null);
  let busy = $state(true);
  let note = $state<string | null>(null);
  let selected = $state<string | null>(null);
  let query = $state('');
  let cursor = $state(0);
  let hiddenNs = $state.raw<ReadonlySet<string>>(new Set());
  let hiddenAreas = $state.raw<ReadonlySet<number>>(new Set());
  let zoom = $state(100);
  let whole = $state(true);

  const shown = $derived(session.schema ? visibleSchema(session.schema, visibilityOf(tab.connectionId)) : null);
  const database = $derived(session.schema?.current_database ?? session.info.server.database);
  const labels = $derived(graph ? areaLabels(graph, m.er_area_loose()) : []);
  const results = $derived(graph && query ? search(graph, query) : []);
  const hits = $derived(graph && query.trim() ? new Set(search(graph, query, Infinity).map((t) => t.id)) : new Set<string>());
  const hidden = $derived.by(() => {
    const out = new Set<string>();
    if (graph) for (const t of graph.tables) if (hiddenNs.has(t.namespace) || hiddenAreas.has(t.area)) out.add(t.id);
    return out;
  });
  const related = $derived(graph && selected ? neighbours(graph, selected) : null);
  const selectedTable = $derived(graph && selected ? (graph.byId.get(selected) ?? null) : null);

  $effect(() => {
    const marks = { selected, related, hits, hidden };
    stage?.setMarks(marks);
  });

  $effect(() => {
    const schema = shown;
    if (schema) void build(schema);
  });

  const aspect = () => {
    const size = stage?.size;
    return size && size.h > 1 ? Math.min(2.4, Math.max(0.8, size.w / size.h)) : 1.4;
  };

  let builds = 0;

  async function build(schema: NonNullable<typeof shown>) {
    const run = ++builds;
    busy = true;
    const g = buildGraph(schema);
    const saved = await api.erLayoutGet(tab.connectionId, database).catch(() => null);
    if (!saved || !applySaved(g, saved)) await autoLayout(g, aspect());
    if (run !== builds) return;
    routeAll(g);
    graph = g;
    selected = selected && g.byId.has(selected) ? selected : null;
    current = scene(g, areaLabels(g, m.er_area_loose()));
    stage?.setScene(current);
    stage?.fit(false);
    busy = false;
    if (!saved) save();
  }

  function save() {
    if (!graph) return;
    api
      .erLayoutSet(tab.connectionId, database, toSaved(graph))
      .then(() => (note = null))
      .catch(() => (note = m.er_save_failed()));
  }

  async function relayout() {
    if (!graph || busy) return;
    busy = true;
    await autoLayout(graph, aspect());
    routeAll(graph);
    stage?.relayout();
    stage?.fit();
    busy = false;
    save();
  }

  function fly(t: ErTable) {
    if (hiddenNs.has(t.namespace)) hiddenNs = new Set([...hiddenNs].filter((n) => n !== t.namespace));
    if (hiddenAreas.has(t.area)) hiddenAreas = new Set([...hiddenAreas].filter((a) => a !== t.area));
    selected = t.id;
    stage?.flyTo(t);
  }

  function pick(t: ErTable | undefined) {
    if (!t) return;
    query = '';
    cursor = 0;
    fly(t);
    host?.focus();
  }

  const uid = $props.id();

  function searchKey(e: KeyboardEvent) {
    if (e.key === 'ArrowDown' || e.key === 'ArrowUp') {
      e.preventDefault();
      const n = results.length;
      if (n) cursor = (cursor + (e.key === 'ArrowDown' ? 1 : n - 1)) % n;
      document.getElementById(`${uid}-r${cursor}`)?.scrollIntoView({ block: 'nearest' });
    } else if (e.key === 'Enter') {
      e.preventDefault();
      pick(results[cursor]);
    } else if (e.key === 'Escape') {
      query = '';
    }
  }

  function focusNeighbours(id: string) {
    const g = graph;
    const t = g?.byId.get(id);
    if (!g || !t) return;
    selected = id;
    const around = [...neighbours(g, id)].flatMap((n) => g.byId.get(n) ?? []);
    stage?.fitTo(around);
  }

  const tableRef = (t: ErTable) => ({ kind: 'table' as const, namespace: t.namespace, name: t.name, table: null, signature: null });

  function nodeMenu(id: string | null, e: MouseEvent) {
    const t = id ? graph?.byId.get(id) : null;
    if (!t) {
      openMenu(e, [
        { label: m.er_fit(), icon: ICON.erFit, run: () => stage?.fit() },
        { label: m.er_relayout(), icon: ICON.erRelayout, disabled: busy, run: () => relayout() },
      ]);
      return;
    }
    openMenu(e, [
      { label: m.tree_open_data(), icon: Table2, run: () => open(t.id) },
      { label: m.tree_new_query(), icon: TextCursorInput, run: () => queryTab(tab.connectionId, selectSql(sqlOf(session.info), t.namespace, t.name, session.schema?.current_database ?? null)) },
      { label: m.er_focus_neighbours(), icon: Network, run: () => focusNeighbours(t.id) },
      { label: m.ddl_show(), icon: FileCode, run: () => showDdl({ connectionId: tab.connectionId, database, dialect: sqlOf(session.info), product: session.info.server.product, object: tableRef(t) }) },
      { label: m.dz_edit_table_dots(), icon: PencilRuler, run: () => void openDesigner(tab.connectionId, database, t.namespace, t.name) },
      ...sqlEntries(tab.connectionId, subjectOf(sqlOf(session.info), database, t.namespace, t.relation, t.name, false)),
      'separator',
      { label: m.tree_copy_name(), icon: ClipboardCopy, hint: t.name, run: () => copyText(t.name) },
      { label: m.tree_copy_create(), run: () => copyDdl(tab.connectionId, database, tableRef(t)) },
    ]);
  }

  $effect(() => {
    const want = tab.erFocus;
    const g = graph;
    if (!want || !g || busy) return;
    tab.erFocus = null;
    const t = g.byId.get(want);
    if (t) fly(t);
  });

  function open(id: string) {
    const t = graph?.byId.get(id);
    if (t) openTable(tab, t.namespace, t.name);
  }

  $effect(() =>
    registerCommands([
      { id: 'er.fit', label: m.pal_cmd_er_fit, group: m.pal_group_view, icon: ICON.erFit, when: () => !!graph, run: () => stage?.fit() },
      { id: 'er.relayout', label: m.pal_cmd_er_relayout, group: m.pal_group_view, icon: ICON.erRelayout, when: () => !!graph && !busy, run: () => relayout() },
      {
        id: 'er.export',
        label: m.pal_cmd_er_export,
        group: m.pal_group_view,
        icon: ICON.export,
        when: () => !!graph,
        run: () => {},
        ask: {
          prompt: m.pal_ask_format,
          choices: () => [
            { id: 'svg', label: m.er_export_svg(), icon: ICON.export },
            { id: 'png', label: m.er_export_png(), icon: ICON.export },
          ],
          run: (v) => exportAs(v === 'png' ? 'png' : 'svg'),
        },
      },
    ]),
  );

  async function exportAs(kind: 'svg' | 'png') {
    if (!current || !host) return;
    try {
      const look = readLook(host);
      const data = kind === 'svg' ? new TextEncoder().encode(toSvg(current, look, hidden)) : await toPng(current, look, hidden);
      await api.exportSave(erFile(database, kind), data);
      note = null;
    } catch {
      note = m.er_export_failed();
    }
  }

  function showArea(index: number) {
    if (!graph) return;
    const g = graph;
    const tables = (g.areas[index]?.tables ?? []).flatMap((id) => g.byId.get(id) ?? []);
    if (hiddenAreas.has(index)) hiddenAreas = new Set([...hiddenAreas].filter((a) => a !== index));
    selected = null;
    stage?.fitTo(tables);
  }

  onMount(() => {
    if (!host || !canvas || !mini) return;
    const h = host;
    stage = new Stage(h, canvas, mini, {
      select: (id) => (selected = id),
      open,
      moved: save,
      menu: nodeMenu,
      view: (v, all) => {
        const z = Math.round(v.k * 100);
        if (z !== zoom) zoom = z;
        if (all !== whole) whole = all;
      },
    });
    const restyle = () => stage?.setLook(readLook(h));
    restyle();
    void document.fonts.ready.then(restyle);
    document.fonts.addEventListener('loadingdone', restyle);
    const observer = new MutationObserver(restyle);
    observer.observe(document.documentElement, { attributes: true, attributeFilter: ['data-style', 'data-palette', 'data-theme'] });
    stage.setMarks({ selected, related, hits, hidden });
    if (current) {
      stage.setScene(current);
      stage.fit(false);
    }
    return () => {
      observer.disconnect();
      document.fonts.removeEventListener('loadingdone', restyle);
      stage?.destroy();
      stage = null;
    };
  });
</script>

<section class="er" data-ready={!busy && graph ? '' : undefined}>
  <div class="er-bar">
    {#if graph}
      <ErFilter {graph} {labels} {hiddenNs} {hiddenAreas} onchange={(ns, areas) => ((hiddenNs = ns), (hiddenAreas = areas))} />
    {/if}
    <div class="er-search">
      <label class="field">
        <Search size={13} strokeWidth={1.8} aria-hidden="true" />
        <input
          type="search"
          placeholder={m.er_search()}
          aria-label={m.er_search()}
          role="combobox"
          aria-expanded={!!query.trim()}
          aria-controls="{uid}-results"
          aria-activedescendant={results.length ? `${uid}-r${cursor}` : undefined}
          aria-autocomplete="list"
          bind:value={query}
          oninput={() => (cursor = 0)}
          onkeydown={searchKey}
        />
      </label>
      {#if query.trim()}
        <ul id="{uid}-results" class="er-results" role="listbox" aria-label={m.er_search()}>
          {#each results as t, i (t.id)}
            <li id="{uid}-r{i}" role="option" aria-selected={i === cursor}>
              <button type="button" tabindex="-1" onpointerdown={(e) => e.preventDefault()} onclick={() => pick(t)}>
                <span class="er-r-name">{t.name}</span>
                <span class="er-r-ns">{t.namespace}</span>
              </button>
            </li>
          {:else}
            <li class="er-r-none" role="option" aria-selected="false" aria-disabled="true">{m.er_results_none()}</li>
          {/each}
        </ul>
      {/if}
    </div>
    <span class="spacer"></span>
    {#if note}<span class="er-note" role="status">{note}</span>{/if}
    <div class="er-zoom" role="group" aria-label={m.er_zoom()}>
      <button class="btn ghost" type="button" aria-label={m.er_zoom_out()} onclick={() => stage?.zoomBy(0.8)}><Minus size={14} strokeWidth={1.8} /></button>
      <button class="btn ghost er-pct" type="button" aria-label={m.er_zoom_reset()} onclick={() => stage?.zoomTo(1)}>{zoom}%</button>
      <button class="btn ghost" type="button" aria-label={m.er_zoom_in()} onclick={() => stage?.zoomBy(1.25)}><Plus size={14} strokeWidth={1.8} /></button>
      <button class="btn ghost" type="button" onclick={() => stage?.fit()}><Maximize size={13} strokeWidth={1.8} aria-hidden="true" />{m.er_fit()}</button>
    </div>
    <button class="btn" type="button" disabled={busy || !graph} onclick={() => void relayout()}>
      <LayoutGrid size={13} strokeWidth={1.8} aria-hidden="true" />
      {m.er_relayout()}
    </button>
    <Menu
      label={m.er_export()}
      disabled={!graph}
      items={[
        { value: 'svg', label: m.er_export_svg() },
        { value: 'png', label: m.er_export_png() },
      ] as const}
      onpick={(v) => void exportAs(v)}
    >
      <Download size={13} strokeWidth={1.8} aria-hidden="true" />
      {m.er_export()}
    </Menu>
  </div>

  <div class="er-main">
    <!-- svelte-ignore a11y_no_noninteractive_tabindex -->
    <div class="er-stage" bind:this={host} tabindex="0" role="application" aria-label={m.er_canvas()} aria-busy={busy}>
      <canvas bind:this={canvas}></canvas>
      <div class="er-mini" class:idle={whole}>
        <canvas bind:this={mini} aria-label={m.er_minimap()}></canvas>
        {#if graph}
          <p class="er-mini-cap">{m.er_count({ tables: graph.tables.length - hidden.size, edges: graph.edges.length })}</p>
        {/if}
      </div>
      <ErLegend />
      {#if graph}
        <dl class="er-title">
          <div><dt>{m.er_title_db()}</dt><dd>{database}</dd></div>
          <div><dt>{m.er_title_tables()}</dt><dd>{graph.tables.length}</dd></div>
          <div><dt>{m.er_title_links()}</dt><dd>{graph.edges.length}</dd></div>
          <div><dt>{m.er_title_scale()}</dt><dd>1 : {(100 / zoom).toFixed(2).replace('.', ',')}</dd></div>
          <div><dt>{m.er_title_sheet()}</dt><dd>ER</dd></div>
        </dl>
      {/if}
      {#if busy}
        <p class="er-msg">{m.er_loading()}</p>
      {:else if graph && graph.tables.length === 0}
        <p class="er-msg">{m.er_empty()}</p>
      {/if}
    </div>
    {#if graph}
      <ErPanel {graph} {labels} {database} table={selectedTable} onselect={fly} onopen={open} onarea={showArea} onclose={() => (selected = null)} />
    {/if}
  </div>
</section>
