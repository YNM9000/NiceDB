<script lang="ts">
  import { siblingPositions } from '../lib/treepos';
  import { Activity, Box, Braces, ChevronRight, Database, Eye, Folder, FunctionSquare, Hash, KeyRound, Layers, Layers2, BookA, Link2, ListTree, Package, ScanSearch, Tags, Waypoints, Table2, TriangleAlert, User, Users, Zap, Shapes } from '@lucide/svelte';
  import { tick, untrack } from 'svelte';
  import type { Component } from 'svelte';
  import { askExport, askImport, killServerSession } from '../lib/tools.svelte';
  import { connections } from '../lib/connections.svelte';
  import { openMenu } from '../lib/contextmenu.svelte';
  import { dbs, visibilityOf } from '../lib/dbs.svelte';
  import { buildNodes, keyOf, parentIndex, typeAhead, windowOf, WINDOW_FROM, type ExNode, type GroupKind } from '../lib/explorer';
  import { menuOf, type MenuContext } from '../lib/explorer-menu';
  import { maintain } from '../lib/monitor.svelte';
  import { explorer, serverOf } from '../lib/explorer-state.svelte';
  import { expandDatabase, hideDatabase, hideSchema, loadServer, loadedSchema, openRelation, queryOn, reloadDatabase, seedOpen, showRelationEr, showTool, showUser, tabOn, toggleDatabase, toggleKey } from '../lib/explorer.svelte';
  import { showToast } from '../lib/toast.svelte';
  import { copyDdl, showDdl } from '../lib/ddl.svelte';
  import { browseText, hasTool, namesOf, relationalCaps, shapeOf, sqlOf } from '../lib/family';
  import { databaseShown, schemaShown, shownDatabases } from '../lib/visibility';
  import { activeTab, databaseOf, loadNamespace, refreshSchema, sessionFor, workspace } from '../lib/workspace.svelte';
  import { compact, tag } from '../lib/format';
  import { selectSql } from '../lib/sqlname';
  import { m } from '../paraglide/messages';
  import type { RelationTag } from '../lib/bindings';
  import { DRIVERS } from '../lib/drivers';
  import DriverMark from './DriverMark.svelte';
  import ColumnCard from './ColumnCard.svelte';
  import { describer, factsOf, type ColumnFacts, type Describe } from '../lib/colinfo';
  import { catalogOf } from '../lib/complete/catalog';
  import { float } from '../lib/float';
  import { placeBeside } from '../lib/menu-place';
  import { api } from '../lib/api';
  import { sentinelMaster } from '../lib/redis';

  let { connectionId }: { connectionId: string } = $props();

  const ROW_TYPEAHEAD_MS = 700;
  const main = $derived(sessionFor(connectionId));
  const caps = $derived(main?.info.caps ?? relationalCaps('postgres'));
  const shape = $derived(shapeOf(caps));
  const tab = $derived(activeTab());
  const focused = $derived(tab?.connectionId === connectionId);
  const current = $derived(focused && tab ? databaseOf(tab) : (workspace.tabs.find((t) => t.connectionId === connectionId)?.database ?? main?.database ?? null));
  const vis = $derived(visibilityOf(connectionId));
  const showAll = $derived(!!dbs.showAll[connectionId]);
  const q = $derived(explorer.filter.trim().toLowerCase());
  const srv = $derived(serverOf(connectionId));
  const databases = $derived(shownDatabases(vis, main?.schema?.databases ?? [], current ?? '', showAll));
  const unsupported = $derived({ users: !hasTool(caps, 'users'), sessions: !hasTool(caps, 'activity') });

  const shownTable = $derived(focused && tab?.view === 'data' && tab.data.info ? { namespace: tab.data.info.namespace, name: tab.data.info.name, database: databaseOf(tab) } : null);

  const nodes = $derived(
    main
      ? buildNodes({
          connection: connectionId,
          shape: { ...shape, server: shape.server && (!unsupported.users || !unsupported.sessions) },
          databases,
          current,
          loaded: (d) => loadedSchema(connectionId, d),
          isOpen: (k) => !!explorer.open[k],
          filter: q,
          databaseDim: (d) => !databaseShown(vis, d),
          schemaShown: (d, n) => showAll || schemaShown(vis, d, n),
          schemaDim: (d, n) => !schemaShown(vis, d, n),
          users: unsupported.users ? null : srv.users,
          sessions: unsupported.sessions ? null : srv.sessions,
          unsupported,
          catalogCount: (d) => explorer.counts[connectionId]?.[d] ?? null,
          pending: (d, n) => sessionFor(connectionId, d)?.pending[n] ?? null,
        })
      : [],
  );

  $effect(() => {
    const sid = main?.info.session_id;
    if (!sid || !shape.databases || !explorer.open[keyOf(connectionId, 'dbs')] || explorer.counts[connectionId] !== undefined) return;
    untrack(() => {
      explorer.counts[connectionId] = null;
      void api
        .databaseCounts(sid)
        .then((rows) => (explorer.counts[connectionId] = Object.fromEntries(rows.map(([d, n]) => [d, n ?? 0]))))
        .catch(() => (explorer.counts[connectionId] = {}));
    });
  });

  $effect(() => {
    const database = current ?? main?.schema?.current_database;
    const schema = database ? loadedSchema(connectionId, database).data : null;
    if (schema && database) untrack(() => seedOpen(connectionId, database, schema, shape.schemas === 'multiple'));
  });

  $effect(() => {
    for (const d of databases) {
      if (explorer.open[keyOf(connectionId, 'db', d)] && loadedSchema(connectionId, d).status === 'idle') untrack(() => void expandDatabase(connectionId, d));
    }
  });

  $effect(() => {
    for (const d of new Set([...databases, current ?? main?.database ?? ''])) {
      const s = sessionFor(connectionId, d);
      for (const [name, p] of Object.entries(s?.pending ?? {})) {
        if (s && p.status === 'idle' && explorer.open[keyOf(connectionId, 'db', d, 'ns', name)]) untrack(() => void loadNamespace(s, name));
      }
    }
  });

  $effect(() => {
    if (!main) return;
    for (const g of ['users', 'sessions'] as const) {
      const open = !!explorer.open[`${connectionId}\u001fsrv\u001f${g}`];
      if (open && srv[g].status === 'idle') untrack(() => void loadServer(connectionId, g));
    }
  });

  let box = $state<HTMLDivElement>();
  let focus = $state(0);
  let view = $state({ above: 0, below: 0, rowPx: 24 });
  let typed = '';
  let typedTimer: ReturnType<typeof setTimeout> | undefined;

  const windowed = $derived(nodes.length > WINDOW_FROM);
  const slice = $derived(windowed ? windowOf(nodes.length, view.rowPx, view.above, view.below) : { start: 0, end: nodes.length });
  const rows = $derived(nodes.slice(slice.start, slice.end));
  const siblings = $derived(siblingPositions(nodes.map((n) => n.depth)));
  const stop = $derived(focus >= slice.start && focus < slice.end ? focus : slice.start);

  function measure() {
    const scroller = box?.closest<HTMLElement>('.conn-list');
    if (!box || !scroller || nodes.length === 0) return;
    const b = box.getBoundingClientRect();
    const s = scroller.getBoundingClientRect();
    const rowPx = b.height / nodes.length;
    if (rowPx > 0) view = { above: s.top - b.top, below: s.bottom - b.top, rowPx };
  }

  $effect(() => {
    const scroller = box?.closest<HTMLElement>('.conn-list');
    if (!scroller) return;
    const watch = new ResizeObserver(() => measure());
    scroller.addEventListener('scroll', measure, { passive: true });
    watch.observe(scroller);
    return () => {
      scroller.removeEventListener('scroll', measure);
      watch.disconnect();
    };
  });

  $effect(() => {
    void nodes.length;
    void tick().then(measure);
  });

  async function focusRow(i: number) {
    const at = Math.max(0, Math.min(nodes.length - 1, i));
    focus = at;
    const scroller = box?.closest<HTMLElement>('.conn-list');
    if (windowed && box && scroller) {
      const b = box.getBoundingClientRect();
      const s = scroller.getBoundingClientRect();
      const top = b.top + at * view.rowPx;
      if (top < s.top) scroller.scrollTop += top - s.top;
      else if (top + view.rowPx > s.bottom) scroller.scrollTop += top + view.rowPx - s.bottom;
      measure();
    }
    await tick();
    box?.querySelector<HTMLElement>(`[data-i="${at}"]`)?.focus();
  }

  const ICONS: Record<GroupKind, Component<{ size?: number; strokeWidth?: number }>> = { tables: Table2, views: Eye, functions: FunctionSquare, sequences: Hash, types: Shapes, triggers: Zap, materialized_views: Layers2, dictionaries: BookA, aliases: Tags, data_streams: Waypoints, packages: Package, synonyms: Link2 };
  const GROUP_LABEL: Record<GroupKind, () => string> = { tables: m.schema_tables, views: m.schema_views, functions: m.schema_functions, sequences: m.ex_sequences, types: m.ex_types, triggers: m.ex_triggers, materialized_views: m.ex_materialized_views, dictionaries: m.ex_dictionaries, aliases: m.ex_aliases, data_streams: m.ex_data_streams, packages: m.ex_packages, synonyms: m.ex_synonyms };

  const timescale = $derived(main?.info.edition?.flavor === 'timescale');
  const platformOf = (n: ExNode) => DRIVERS.find((d) => d.id === n.platform) ?? null;

  function tagLabel(t: RelationTag): string {
    if (t.kind === 'hypertable') return m.tag_hyper();
    if (t.kind === 'continuous_aggregate') return m.tag_cagg();
    return m.tag_rls();
  }

  function tagTip(t: RelationTag): string {
    if (t.kind === 'hypertable') return m.tag_hyper_tip({ chunks: compact(t.chunks), compressed: compact(t.compressed) });
    if (t.kind === 'continuous_aggregate') return m.tag_cagg_tip();
    return t.policies === 1 ? m.tag_rls_tip_one() : m.tag_rls_tip({ count: t.policies });
  }

  function labelOf(n: ExNode): string {
    if (n.kind === 'databases') return m.ex_databases();
    if (n.kind === 'platform') return platformOf(n)?.name ?? n.label;
    if (n.kind === 'group' && n.path.group === 'materialized_views' && timescale) return m.ex_continuous_aggregates();
    if (n.kind === 'schemas') return m.ex_schemas();
    if (n.kind === 'group' && n.path.group === 'tables' && caps.words.table === 'collection') return m.schema_collections();
    if (n.kind === 'group' && n.path.group === 'tables' && caps.words.table === 'key') return m.schema_prefixes();
    if (n.kind === 'group' && n.path.group === 'tables' && caps.words.table === 'index') return m.schema_indices();
    if (n.kind === 'group' && n.path.group) return GROUP_LABEL[n.path.group]();
    if (n.kind === 'server') return n.path.server === 'users' ? m.ex_users() : m.ex_sessions();
    if (n.kind === 'note' && n.busy) return m.ex_loading();
    return n.label;
  }

  function iconOf(n: ExNode): Component<{ size?: number; strokeWidth?: number }> | null {
    switch (n.kind) {
      case 'databases':
        return Layers;
      case 'database':
        return Database;
      case 'schemas':
        return ListTree;
      case 'schema':
        return Box;
      case 'group':
        if (n.path.group === 'tables' && caps.words.table === 'key') return KeyRound;
        if (n.path.group === 'tables' && caps.words.table === 'index') return ScanSearch;
        return n.path.group === 'tables' && caps.words.table === 'collection' ? Braces : n.path.group ? ICONS[n.path.group] : null;
      case 'relation':
        if (caps.words.table === 'key') return Folder;
        if (n.path.group === 'tables' && caps.words.table === 'index') return ScanSearch;
        return n.path.group === 'views' ? Eye : n.path.group && n.path.group !== 'tables' ? ICONS[n.path.group] : caps.words.table === 'collection' ? Braces : Table2;
      case 'routine':
        return FunctionSquare;
      case 'object':
        return n.path.group ? ICONS[n.path.group] : null;
      case 'column':
        return n.active ? KeyRound : null;
      case 'server':
        return n.path.server === 'users' ? Users : Activity;
      case 'principal':
        return User;
      case 'note':
        return n.failed ? TriangleAlert : null;
      default:
        return null;
    }
  }

  let card = $state<{ key: string; anchor: HTMLElement; facts: ColumnFacts; describe: Describe | null } | null>(null);

  function columnInfo(n: ExNode) {
    const p = n.path;
    if (card?.key === n.key) {
      card = null;
      return;
    }
    const s = p.database ? sessionFor(connectionId, p.database) : main;
    const catalog = catalogOf(s?.schema ?? null);
    const rel = catalog?.find([p.namespace ?? '', p.name ?? ''])[0];
    const facts = catalog && rel && p.id ? factsOf(catalog, rel, p.id) : null;
    const anchor = box?.querySelector<HTMLElement>(`[data-i="${nodes.indexOf(n)}"]`);
    if (!s || !facts || !anchor) return;
    card = { key: n.key, anchor, facts, describe: s.info.dialect ? describer(s.info.session_id, s.schema, api.tableDesign) : null };
  }

  $effect(() => {
    if (!card) return;
    const close = (e: KeyboardEvent) => {
      if (e.key === 'Escape') card = null;
    };
    window.addEventListener('keydown', close);
    return () => window.removeEventListener('keydown', close);
  });

  async function activate(n: ExNode) {
    const p = n.path;
    if (n.kind === 'column') {
      columnInfo(n);
      return;
    }
    if (n.kind === 'database' && p.database) {
      if (!explorer.open[n.key]) {
        explorer.open[n.key] = true;
        void expandDatabase(connectionId, p.database);
      }
      await tabOn(connectionId, p.database);
    } else if (n.kind === 'relation' && p.database && p.namespace && p.name) {
      await openRelation(connectionId, p.database, p.namespace, p.name);
    } else if (n.kind === 'principal' && p.name) {
      await showUser(connectionId, p.name);
    } else if (n.expandable) {
      await toggle(n);
    }
  }

  async function toggle(n: ExNode) {
    if (n.kind === 'database' && n.path.database) await toggleDatabase(connectionId, n.path.database);
    else toggleKey(n.key);
  }

  async function refresh(n: ExNode) {
    const p = n.path;
    if (p.server) await loadServer(connectionId, p.server);
    else if (p.database) await reloadDatabase(connectionId, p.database);
    else await refreshSchema(connectionId);
  }

  const act: MenuContext['act'] = {
    toggle: (n) => void toggle(n),
    activate: (n) => void activate(n),
    refresh: (n) => void refresh(n),
    query: (database, sql) => void queryOn(connectionId, database, sql),
    openData: (p) => void (p.database && p.namespace && p.name && openRelation(connectionId, p.database, p.namespace, p.name)),
    er: (p) => void (p.database && p.namespace && p.name && showRelationEr(connectionId, p.database, p.namespace, p.name)),
    tool: (database, kind) => void showTool(connectionId, database, kind),
    user: (key) => void showUser(connectionId, key),
    maintain: (database, action, table) => void maintain(connectionId, database, action, table),
    exportTable: (p) => {
      const rel = main && p.database ? loadedSchema(connectionId, p.database).data?.namespaces.find((n) => n.name === p.namespace) : null;
      const found = [...(rel?.tables ?? []), ...(rel?.views ?? [])].find((r) => r.name === p.name);
      if (p.namespace && p.name) askExport({ connectionId, database: p.database, source: { kind: 'table', namespace: p.namespace, name: p.name }, label: p.name, expected: found?.estimated_rows ?? null });
    },
    importTable: (p) => void (p.namespace && p.name && askImport({ connectionId, database: p.database, table: { namespace: p.namespace, name: p.name } })),
    hideDatabase: (d, shown) => hideDatabase(connectionId, d, shown),
    hideSchema: (d, n, shown) => hideSchema(connectionId, d, n, shown),
    kill: (id, query, mode) => void kill(id, query, mode),
    ddl: (database, object) => main && showDdl({ connectionId, database, dialect: main.info.dialect ?? null, product: main.info.server.product, object }),
    copyDdl: (database, object) => void copyDdl(connectionId, database, object),
    columnInfo: (n) => columnInfo(n),
  };

  async function kill(id: string, query: string | null, mode: 'cancel' | 'terminate') {
    if (!main) return;
    const said = await killServerSession(main.info, { id, query }, mode);
    if (said === null) return;
    showToast(said);
    await loadServer(connectionId, 'sessions');
  }

  const context = (): MenuContext => ({
    connectionId,
    dialect: main ? (namesOf(main.info) ?? sqlOf(main.info)) : 'postgres',
    info: main?.info ?? null,
    schema: (d) => loadedSchema(connectionId, d),
    databaseShown: (d) => databaseShown(vis, d),
    schemaShown: (d, n) => schemaShown(vis, d, n),
    isOpen: (k) => !!explorer.open[k],
    act,
  });

  function menu(e: MouseEvent, n: ExNode) {
    const at = nodes.indexOf(n);
    if (at >= 0) focus = at;
    openMenu(e, menuOf(context(), n));
  }

  function click(n: ExNode, e: MouseEvent) {
    focus = nodes.indexOf(n);
    if ((e.target as HTMLElement).closest('.ex-chev')) {
      if (n.expandable) void toggle(n);
      return;
    }
    void activate(n);
  }

  function dblclick(n: ExNode) {
    const p = n.path;
    if (n.kind === 'relation' && p.database && p.namespace && p.name) {
      const { namespace, name, database } = p;
      void queryOn(connectionId, database, browseText(caps, name, () => selectSql(main ? sqlOf(main.info) : 'postgres', namespace, name, database)));
    }
  }

  function key(e: KeyboardEvent) {
    const row = (e.target as HTMLElement).closest<HTMLElement>('[data-i]');
    if (!row) return;
    const i = Number(row.dataset.i);
    const n = nodes[i];
    if (!n || e.ctrlKey || e.metaKey || e.altKey) return;
    let handled = true;
    if (e.key.length > 1 || e.key === ' ') typed = '';
    if (e.key === 'ArrowDown') void focusRow(i + 1);
    else if (e.key === 'ArrowUp') {
      if (i === 0) box?.closest('.conn-list')?.querySelector<HTMLElement>(`[data-conn="${connectionId}"] .conn-main`)?.focus();
      else void focusRow(i - 1);
    } else if (e.key === 'Home') void focusRow(0);
    else if (e.key === 'End') void focusRow(nodes.length - 1);
    else if (e.key === 'ArrowRight') {
      if (n.expandable && !n.open) void toggle(n);
      else if (n.open) void focusRow(i + 1);
    } else if (e.key === 'ArrowLeft') {
      if (n.expandable && n.open) void toggle(n);
      else void focusRow(parentIndex(nodes, i));
    } else if (e.key === 'Enter') void activate(n);
    else if (e.key === ' ') {
      if (n.expandable) void toggle(n);
    } else if (e.key.length === 1) {
      typed += e.key;
      clearTimeout(typedTimer);
      typedTimer = setTimeout(() => (typed = ''), ROW_TYPEAHEAD_MS);
      const hit = typeAhead(nodes, i, typed);
      if (hit >= 0) void focusRow(hit);
    } else handled = false;
    if (handled) e.preventDefault();
  }

  const name = $derived(connections.list.find((c) => c.id === connectionId)?.name ?? '');
  const sentinel = $derived(main && caps.family === 'key_value' ? sentinelMaster(main.info.server.database) : null);
  const clock = (t: number) => new Date(t).toLocaleTimeString(tag(), { hour: '2-digit', minute: '2-digit' });
</script>

{#if main}
  {#if sentinel}
    <div class="ex-sentinel" role="note" data-tip={m.sentinel_tip()}>
      <span class="caps">{m.sentinel_label()}</span>
      <span class="mono">{sentinel.name}</span>
      <span class="ex-sentinel-to mono">→ {sentinel.address}</span>
      {#if main.failover}<span class="ex-sentinel-moved">{m.sentinel_moved({ time: clock(main.failover.at), from: main.failover.from })}</span>{/if}
    </div>
  {/if}
  <div class="ex" class:windowed bind:this={box} role="tree" tabindex="-1" aria-label={m.ex_aria({ name })} data-ex={connectionId} style:height={windowed ? `calc(var(--nd-ex-row, 24px) * ${nodes.length})` : undefined}>
    <div class="ex-rows" style:padding-top={windowed ? `calc(var(--nd-ex-row, 24px) * ${slice.start})` : undefined}>
      {#each rows as n, r (n.key)}
        {@const i = slice.start + r}
        {@const Icon = iconOf(n)}
        <div
          class="ex-row ex-{n.kind}"
          class:on={n.kind === 'relation' && !!shownTable && shownTable.name === n.path.name && shownTable.namespace === n.path.namespace && shownTable.database === n.path.database}
          class:active={n.kind === 'database' && n.active}
          class:dim={n.dim}
          class:failed={n.failed}
          class:busy={n.busy}
          class:holder={n.kind === 'databases' || n.kind === 'schemas' || n.kind === 'group' || n.kind === 'platform'}
          class:focus={i === focus}
          role="treeitem"
          aria-level={n.depth + 1}
          aria-setsize={siblings.size[i]}
          aria-posinset={siblings.pos[i]}
          aria-expanded={n.expandable ? n.open : undefined}
          aria-selected={n.kind === 'database' ? n.active : undefined}
          data-i={i}
          data-kind={n.kind}
          tabindex={i === stop ? 0 : -1}
          style:--depth={n.depth}
          data-tip={n.kind === 'relation' ? (n.detail ? `${n.detail}\n${m.tree_row_title()}` : m.tree_row_title()) : n.kind === 'platform' ? m.ex_managed_tip({ product: labelOf(n) }) : n.failed ? n.label : undefined}
          onclick={(e) => click(n, e)}
          onkeydown={key}
          ondblclick={() => dblclick(n)}
          oncontextmenu={(e) => menu(e, n)}
          onfocus={() => (focus = i)}
        >
          <span class="ex-chev" class:open={n.open} class:none={!n.expandable} aria-hidden="true"><ChevronRight size={12} strokeWidth={2} /></span>
          <span class="ex-icon" aria-hidden="true">{#if n.kind === 'platform' && platformOf(n)}<DriverMark driver={platformOf(n)!} size={12} />{:else if Icon}<Icon size={13} strokeWidth={1.6} />{/if}</span>
          <span class="tree-row ex-label"><span class="tree-name">{labelOf(n)}</span></span>
          {#if n.tags}{#each n.tags as t, k (k)}<span class="ex-tag" class:hyper={t.kind === 'hypertable'} data-tip={tagTip(t)} role="note" aria-label={tagTip(t)}>{tagLabel(t)}</span>{/each}{/if}
          {#if n.detail && n.kind !== 'note' && n.kind !== 'relation'}<span class="col-type ex-detail">{n.detail}</span>{/if}
          {#if n.kind === 'relation' && n.count}<span class="tree-count">{caps.words.table === 'key' ? (n.count === 1 ? m.schema_key_one() : m.schema_keys({ count: compact(n.count) })) : m.schema_rows({ count: compact(n.count) })}</span>{:else if n.count !== null && n.kind !== 'relation'}<span class="tree-count">{n.count}</span>{/if}
          {#if n.kind === 'database' && n.active && focused}<span class="ex-live" role="img" aria-label={m.ex_current()}></span>{/if}
        </div>
      {/each}
    </div>
  </div>
  {#if card}
    <div class="ex-card pop-panel" role="dialog" aria-label={m.colinfo_open()} use:float={{ anchor: card.anchor, place: (a, size, view) => placeBeside(a, size, view, 0), onaway: () => (card = null) }}>
      <ColumnCard facts={card.facts} describe={card.describe} />
    </div>
  {/if}
{/if}
