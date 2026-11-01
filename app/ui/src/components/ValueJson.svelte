<script lang="ts">
  import { ChevronRight, ClipboardCopy, Copy, Database } from '@lucide/svelte';
  import { tick } from 'svelte';
  import type { Dialect } from '../lib/bindings';
  import { copyText, keys, openMenu } from '../lib/contextmenu.svelte';
  import { num } from '../lib/format';
  import { at, children, initialOpen, jsonPath, pathKey, scalarText, search, sqlPath, typeOf, type Path, type Seg } from '../lib/jsonpath';
  import { m } from '../paraglide/messages';
  import { bsonType, wrapperText, wrapperType } from '../lib/ejson';

  interface Props {
    value: unknown;
    query: string;
    column: string;
    dialect: Dialect | null;
    bson?: boolean;
  }
  let { value, query, column, dialect, bson = false }: Props = $props();
  const kidsOf = (v: unknown) => (bson && wrapperType(v) ? [] : children(v));

  const PAGE = 100;
  const MAX_ROWS = 20_000;

  type Row = { kind: 'node'; path: Path; key: string; depth: number; value: unknown; size: number } | { kind: 'more'; path: Path; depth: number; left: number };

  let open = $state.raw(new Set<string>());
  let closed = $state.raw(new Set<string>());
  let pages = $state.raw(new Map<string, number>());
  let active = $state<string>(pathKey([]));
  let list = $state<HTMLElement>();

  $effect(() => {
    open = initialOpen(value);
    pages = new Map();
    active = pathKey([]);
  });

  const found = $derived(search(value, query));
  const hits = $derived([...found.hits]);
  const visibleOpen = $derived(query.trim() ? new Set([...open, ...found.open].filter((k) => !closed.has(k))) : open);

  $effect(() => {
    void query;
    closed = new Set();
  });

  const rows = $derived.by(() => {
    const out: Row[] = [];
    const walk = (v: unknown, path: Seg[], key: string, depth: number) => {
      if (out.length >= MAX_ROWS) return;
      const kids = kidsOf(v);
      out.push({ kind: 'node', path, key, depth, value: v, size: kids.length });
      if (!kids.length || !visibleOpen.has(pathKey(path))) return;
      const limit = pages.get(pathKey(path)) ?? PAGE;
      for (const [k, x] of kids.slice(0, limit)) walk(x, [...path, k], typeof k === 'number' ? String(k) : k, depth + 1);
      if (kids.length > limit) out.push({ kind: 'more', path, depth: depth + 1, left: kids.length - limit });
    };
    walk(value, [], '', 0);
    return out;
  });

  const nodes = $derived(rows.filter((r): r is Extract<Row, { kind: 'node' }> => r.kind === 'node'));
  const activeRow = $derived(nodes.find((r) => pathKey(r.path) === active) ?? nodes[0]);
  const activePath = $derived(activeRow?.path ?? []);
  const hitIndex = $derived(hits.indexOf(active));

  function toggle(path: Path, to?: boolean) {
    const k = pathKey(path);
    const next = new Set(open);
    const shut = new Set(closed);
    if (to ?? !visibleOpen.has(k)) {
      next.add(k);
      shut.delete(k);
    } else {
      next.delete(k);
      shut.add(k);
    }
    open = next;
    closed = shut;
  }

  function expandAll(path: Path) {
    const next = new Set(open);
    const walk = (v: unknown, p: Seg[]) => {
      if (next.size > 5000) return;
      if (children(v).length) next.add(pathKey(p));
      for (const [k, x] of children(v)) walk(x, [...p, k]);
    };
    walk(at(value, path), [...path]);
    open = next;
  }

  function collapseAll(path: Path) {
    const self = pathKey(path);
    const inside = path.length ? `${self.slice(0, -1)},` : '[';
    open = new Set([...open].filter((k) => k === '[]' || !(k === self || k.startsWith(inside))));
  }

  async function focusActive() {
    await tick();
    const el = list?.querySelector<HTMLElement>('.vj-row.active');
    el?.scrollIntoView({ block: 'nearest' });
    el?.focus({ preventScroll: true });
  }

  export function next(step = 1) {
    if (!hits.length) return;
    const i = hitIndex < 0 ? (step > 0 ? 0 : hits.length - 1) : (hitIndex + step + hits.length) % hits.length;
    active = hits[i] ?? active;
    void focusActive();
  }

  export const hitText = () => (hits.length ? `${hitIndex < 0 ? '–' : num(hitIndex + 1)}/${num(found.count)}` : query.trim() ? m.vp_json_no_hits() : '');

  const valueText = (v: unknown) => (typeOf(v) === 'object' || typeOf(v) === 'array' ? JSON.stringify(v, null, 2) : typeof v === 'string' ? v : String(v));

  function menu(e: MouseEvent, path: Path) {
    e.preventDefault();
    active = pathKey(path);
    const v = at(value, path);
    const container = children(v).length > 0;
    openMenu(e, [
      { label: m.vp_json_copy_path(), icon: ClipboardCopy, hint: jsonPath(path), shortcut: keys('Shift+Mod+C'), run: () => copyText(jsonPath(path)) },
      !!dialect && !!column && { label: m.vp_json_copy_sql(), icon: Database, hint: sqlPath(dialect, column, path), run: () => copyText(sqlPath(dialect, column, path)) },
      { label: m.vp_json_copy_value(), icon: Copy, shortcut: keys('Mod+C'), run: () => copyText(valueText(v)) },
      container && 'separator',
      container && { label: m.vp_json_expand_all(), run: () => expandAll(path) },
      container && { label: m.vp_json_collapse_all(), run: () => collapseAll(path) },
    ]);
  }

  function onkeydown(e: KeyboardEvent) {
    const i = nodes.findIndex((r) => pathKey(r.path) === active);
    const row = nodes[i];
    if (!row) return;
    const mod = e.ctrlKey || e.metaKey;
    const go = (to: number) => {
      const r = nodes[Math.max(0, Math.min(nodes.length - 1, to))];
      if (r) active = pathKey(r.path);
      void focusActive();
    };
    const isOpen = visibleOpen.has(active);
    if (e.key === 'ArrowDown') go(i + 1);
    else if (e.key === 'ArrowUp') go(i - 1);
    else if (e.key === 'Home') go(0);
    else if (e.key === 'End') go(nodes.length - 1);
    else if (e.key === 'ArrowRight' && row.size) isOpen ? go(i + 1) : toggle(row.path, true);
    else if (e.key === 'ArrowLeft') {
      if (row.size && isOpen) toggle(row.path, false);
      else if (row.path.length) {
        active = pathKey(row.path.slice(0, -1));
        void focusActive();
      }
    } else if ((e.key === 'Enter' || e.key === ' ') && row.size) toggle(row.path);
    else if (mod && e.shiftKey && e.key.toLowerCase() === 'c') void copyText(jsonPath(row.path));
    else if (mod && e.key.toLowerCase() === 'c') void copyText(valueText(row.value));
    else return;
    e.preventDefault();
    e.stopPropagation();
  }

  function preview(v: unknown): string {
    const parts: string[] = [];
    for (const [k, x] of children(v).slice(0, 4)) {
      const t = typeOf(x);
      const shown = bson && wrapperType(x) ? wrapperText(x) : t === 'object' ? '{…}' : t === 'array' ? '[…]' : scalarText(x);
      parts.push(Array.isArray(v) ? shown : `${k}: ${shown}`);
    }
    const more = children(v).length > 4 ? ', …' : '';
    return Array.isArray(v) ? `[${parts.join(', ')}${more}]` : `{${parts.join(', ')}${more}}`;
  }
</script>

<!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
<div class="vj" role="tree" tabindex="-1" aria-label={m.vp_json_tree()} bind:this={list} {onkeydown}>
  {#each rows as row, i (row.kind === 'node' ? pathKey(row.path) : `more:${pathKey(row.path)}:${i}`)}
    {#if row.kind === 'more'}
      <button class="vj-more" type="button" style:--d={row.depth} onclick={() => (pages = new Map(pages).set(pathKey(row.path), (pages.get(pathKey(row.path)) ?? PAGE) + PAGE * 5))}>{m.vp_json_more({ count: num(row.left) })}</button>
    {:else}
      {@const k = pathKey(row.path)}
      {@const t = typeOf(row.value)}
      {@const isOpen = row.size > 0 && visibleOpen.has(k)}
      <!-- svelte-ignore a11y_click_events_have_key_events -->
      <div
        class="vj-row"
        class:active={k === active}
        class:hit={found.hits.has(k)}
        style:--d={row.depth}
        role="treeitem"
        aria-level={row.depth + 1}
        aria-expanded={row.size ? isOpen : undefined}
        aria-selected={k === active}
        tabindex={k === active ? 0 : -1}
        onclick={() => (active = k)}
        ondblclick={() => row.size && toggle(row.path)}
        oncontextmenu={(e) => menu(e, row.path)}
      >
        {#if row.size}
          <button class="vj-tog" type="button" tabindex="-1" aria-label={isOpen ? m.vp_json_collapse() : m.vp_json_expand()} onclick={(e) => (e.stopPropagation(), (active = k), toggle(row.path))}><ChevronRight size={12} strokeWidth={2} class={isOpen ? 'open' : ''} /></button>
        {:else}<span class="vj-tog" aria-hidden="true"></span>{/if}
        {#if row.depth > 0}<span class="vj-key" class:idx={typeof row.path.at(-1) === 'number'}>{row.key}</span><span class="vj-colon" aria-hidden="true">:</span>{/if}
        {#if row.size}
          <span class="vj-sum">{t === 'array' ? `[${num(row.size)}]` : `{${num(row.size)}}`}</span>
          {#if !isOpen}<span class="vj-peek">{preview(row.value)}</span>{/if}
        {:else if bson && wrapperType(row.value)}<span class="vj-v vj-bson">{wrapperText(row.value)}</span>
        {:else if t === 'object'}<span class="vj-sum">{'{}'}</span>
        {:else if t === 'array'}<span class="vj-sum">[]</span>
        {:else}<span class="vj-v vj-{t}">{scalarText(row.value)}</span>{/if}
        {#if bson}<span class="vj-type">{bsonType(row.value)}</span>{/if}
        <button class="vj-copy" type="button" tabindex="-1" data-tip={`${m.vp_json_copy_path()} · ${jsonPath(row.path)}`} aria-label={m.vp_json_copy_path()} onclick={(e) => (e.stopPropagation(), void copyText(jsonPath(row.path)))}><ClipboardCopy size={12} strokeWidth={1.8} /></button>
      </div>
    {/if}
  {/each}
</div>
<footer class="vj-path">
  <code data-tip={jsonPath(activePath)}>{jsonPath(activePath)}</code>
  <button class="icon-btn" type="button" aria-label={m.vp_json_copy_path()} data-tip={m.vp_json_copy_path()} onclick={() => void copyText(jsonPath(activePath))}><ClipboardCopy size={14} strokeWidth={1.8} /></button>
  {#if dialect && column}<button class="btn ghost" type="button" data-tip={sqlPath(dialect, column, activePath)} onclick={() => void copyText(sqlPath(dialect, column, activePath))}><Database size={13} strokeWidth={1.8} aria-hidden="true" />SQL</button>{/if}
</footer>
