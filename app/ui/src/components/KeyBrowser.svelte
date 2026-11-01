<script lang="ts">
  import { siblingPositions } from '../lib/treepos';
  import { ChevronRight, ClipboardCopy, Filter, Folder, FolderOpen, Plus, RefreshCw, Search, TextCursorInput, Trash2, X } from '@lucide/svelte';
  import { onMount, tick, untrack } from 'svelte';
  import { copyText, openMenu } from '../lib/contextmenu.svelte';
  import { compact, num } from '../lib/format';
  import { KINDS, keyTree, shortTtl, type KeyRow } from '../lib/keys';
  import { folderPattern, keyBrowser, LOAD_ALL_CAP, writeKey } from '../lib/keys.svelte';
  import { registerCommands } from '../lib/palette.svelte';
  import { commandLine } from '../lib/redis';
  import { showToast } from '../lib/toast.svelte';
  import { bytes } from '../lib/tools';
  import { newTab, type sessionOf, type Tab } from '../lib/workspace.svelte';
  import { m } from '../paraglide/messages';
  import KeyDetail from './KeyDetail.svelte';
  import KeyNew from './KeyNew.svelte';
  import Select from './Select.svelte';

  let { tab, session }: { tab: Tab; session: NonNullable<ReturnType<typeof sessionOf>> } = $props();

  const sid = $derived(session.info.session_id);
  const st = $derived(keyBrowser(tab.id));
  const rows = $derived(keyTree(st.keys, st.open));
  const writable = $derived(session.info.caps.editable);

  let ROW = $state(24);
  const OVERSCAN = 16;
  let list = $state<HTMLElement>();
  let scrollTop = $state(0);
  let height = $state(400);
  let cursor = $state(0);
  let creating = $state(false);

  const first = $derived(Math.max(0, Math.floor(scrollTop / ROW) - OVERSCAN));
  const last = $derived(Math.min(rows.length, Math.ceil((scrollTop + height) / ROW) + OVERSCAN));
  const shown = $derived(rows.slice(first, last));
  const siblings = $derived(siblingPositions(rows.map((r) => r.depth)));

  const KIND_LABEL: Record<string, () => string> = {
    string: m.kv_type_string,
    hash: m.kv_type_hash,
    list: m.kv_type_list,
    set: m.kv_type_set,
    zset: m.kv_type_zset,
    stream: m.kv_type_stream,
    'ReJSON-RL': m.kv_type_json,
  };
  const SHORT: Record<string, string> = { string: 'str', hash: 'hash', list: 'list', set: 'set', zset: 'zset', stream: 'xs', 'ReJSON-RL': 'json' };
  const kindName = (k: string) => KIND_LABEL[k]?.() ?? k;

  onMount(() => {
    if (!list) return;
    ROW = parseFloat(getComputedStyle(list).getPropertyValue('--nd-kv-row')) || ROW;
    const watch = new ResizeObserver(() => (height = list?.clientHeight ?? height));
    watch.observe(list);
    return () => watch.disconnect();
  });

  let seenTable: Tab['table'] | undefined = undefined;
  $effect(() => {
    const t = tab.table;
    const s = st;
    untrack(() => {
      if (t && t !== seenTable) {
        seenTable = t;
        s.pattern = folderPattern(t.name);
        s.open.add(`${t.name}:`);
        void s.reload(sid);
      } else if (!s.started && !s.scanning) {
        seenTable = t;
        void s.reload(sid);
      }
    });
  });

  function search() {
    cursor = 0;
    void st.reload(sid);
  }

  function clearSearch() {
    st.pattern = '';
    st.kind = '';
    search();
  }

  async function go(i: number) {
    cursor = Math.max(0, Math.min(rows.length - 1, i));
    await tick();
    const top = cursor * ROW;
    if (list && top < list.scrollTop) list.scrollTop = top;
    else if (list && top + ROW > list.scrollTop + list.clientHeight) list.scrollTop = top + ROW - list.clientHeight;
    list?.querySelector<HTMLElement>(`[data-row="${cursor}"]`)?.focus({ preventScroll: true });
  }

  function activate(row: KeyRow, i: number) {
    cursor = i;
    if (row.kind === 'folder') st.toggle(row.path);
    else void st.select(sid, row.path);
  }

  function onkeydown(e: KeyboardEvent) {
    const row = rows[cursor];
    const page = Math.max(1, Math.floor(height / ROW) - 1);
    const to = { ArrowDown: cursor + 1, ArrowUp: cursor - 1, PageDown: cursor + page, PageUp: cursor - page, Home: 0, End: rows.length - 1 }[e.key];
    if (to !== undefined) {
      e.preventDefault();
      void go(to).then(() => {
        const r = rows[cursor];
        if (r?.kind === 'key') void st.select(sid, r.path);
      });
      return;
    }
    if (!row) return;
    if (e.key === 'ArrowRight' && row.kind === 'folder' && !row.open) st.toggle(row.path);
    else if (e.key === 'ArrowLeft' && row.kind === 'folder' && row.open) st.toggle(row.path);
    else if (e.key === 'Enter') activate(row, cursor);
    else if (e.key === 'Delete' && row.kind === 'key' && writable) void remove(row.path);
    else return;
    e.preventDefault();
  }

  async function remove(key: string) {
    const write = { op: 'delete' as const, keys: [key] };
    const done = await writeKey(session.info, write);
    if (!done) return;
    if (!done.ok) return showToast(done.message, null, 'error');
    st.applied(write);
    showToast(m.kv_deleted({ key }));
  }

  function readCommand(row: KeyRow): string {
    const k = row.path;
    const verb = { hash: ['HGETALL', k], list: ['LRANGE', k, '0', '99'], set: ['SSCAN', k, '0', 'COUNT', '100'], zset: ['ZRANGE', k, '0', '99', 'REV', 'WITHSCORES'], stream: ['XRANGE', k, '-', '+', 'COUNT', '100'], 'ReJSON-RL': ['JSON.GET', k] }[row.info?.kind ?? ''] ?? ['GET', k];
    return commandLine(verb);
  }

  function menu(e: MouseEvent, row: KeyRow, i: number) {
    e.preventDefault();
    cursor = i;
    if (row.kind === 'folder') {
      openMenu(e, [
        { label: row.open ? m.tree_collapse() : m.tree_expand(), run: () => st.toggle(row.path) },
        { label: m.kv_filter_folder(), icon: Filter, hint: folderPattern(row.path), run: () => ((st.pattern = folderPattern(row.path)), st.open.add(row.path), search()) },
        'separator',
        { label: m.kv_copy_prefix(), icon: ClipboardCopy, hint: row.path, run: () => copyText(row.path) },
      ]);
      return;
    }
    void st.select(sid, row.path);
    openMenu(e, [
      { label: m.kv_copy_name(), icon: ClipboardCopy, hint: row.path, run: () => copyText(row.path) },
      { label: m.kv_open_in_editor(), icon: TextCursorInput, run: () => newTab(tab.connectionId, readCommand(row)) },
      writable && 'separator',
      writable && { label: m.kv_delete(), icon: Trash2, danger: true, run: () => void remove(row.path) },
    ]);
  }

  $effect(() =>
    registerCommands([
      { id: 'keys.new', label: m.kv_new_key, group: m.pal_group_query, when: () => writable && tab.view === 'data', run: () => (creating = true) },
      { id: 'keys.refresh', label: m.kv_refresh, group: m.pal_group_query, when: () => tab.view === 'data', run: search },
      { id: 'keys.load_all', label: m.kv_load_all, group: m.pal_group_query, when: () => tab.view === 'data' && !st.done, run: () => void st.all(sid) },
    ]),
  );

  const loadedLine = $derived(st.total === null ? m.kv_loaded({ count: num(st.keys.length) }) : m.kv_loaded_of({ count: num(st.keys.length), total: num(st.total) }));
</script>

<div class="kv">
  <div class="kv-side">
    <form class="kv-head" onsubmit={(e) => (e.preventDefault(), search())}>
      <label class="field kv-find">
        <Search size={13} strokeWidth={1.8} aria-hidden="true" />
        <input bind:value={st.pattern} placeholder={m.kv_pattern_hint()} aria-label={m.kv_pattern()} spellcheck="false" autocomplete="off" />
        {#if st.pattern || st.kind}<button class="kv-clear" type="button" aria-label={m.kv_clear()} data-tip={m.kv_clear()} onclick={clearSearch}><X size={12} strokeWidth={1.8} /></button>{/if}
      </label>
      <Select class="tool-select kv-kind" label={m.kv_type()} bind:value={st.kind} onchange={search} options={[{ value: '', label: m.kv_all_types() }, ...KINDS.map((k) => ({ value: k, label: kindName(k) }))]} />
      <button class="icon-btn" type="submit" aria-label={m.kv_refresh()} data-tip={m.kv_refresh()}><RefreshCw size={13} strokeWidth={1.8} /></button>
      {#if writable}
        <button class="icon-btn" type="button" aria-label={m.kv_new_key()} data-tip={m.kv_new_key()} onclick={() => (creating = true)}><Plus size={14} strokeWidth={1.8} /></button>
      {/if}
    </form>

    <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
    <div class="kv-list" role="tree" tabindex="-1" aria-label={m.kv_keys()} bind:this={list} onscroll={() => (scrollTop = list?.scrollTop ?? 0)} {onkeydown}>
      {#if st.error}
        <p class="kv-empty err" role="alert">{st.error}</p>
      {:else if st.done && rows.length === 0}
        <p class="kv-empty">{st.pattern || st.kind ? m.kv_none_match() : m.kv_none()}</p>
      {/if}
      <div class="kv-spacer" style:height="{rows.length * ROW}px">
        {#each shown as row, j (row.kind + row.path)}
          {@const i = first + j}
          <div
            class="kv-row"
            class:folder={row.kind === 'folder'}
            role="treeitem"
            aria-level={row.depth + 1}
            aria-setsize={siblings.size[i]}
            aria-posinset={siblings.pos[i]}
            aria-expanded={row.kind === 'folder' ? row.open : undefined}
            aria-selected={row.kind === 'key' && row.path === st.selected}
            tabindex={i === cursor ? 0 : -1}
            data-row={i}
            style:top="{i * ROW}px"
            style:--kv-depth={row.depth}
            onclick={() => activate(row, i)}
            oncontextmenu={(e) => menu(e, row, i)}
            onkeydown={() => {}}
          >
            {#if row.kind === 'folder'}
              <span class="kv-chev" class:open={row.open} aria-hidden="true"><ChevronRight size={12} strokeWidth={1.8} /></span>
              {#if row.open}<FolderOpen size={13} strokeWidth={1.7} aria-hidden="true" />{:else}<Folder size={13} strokeWidth={1.7} aria-hidden="true" />{/if}
              <span class="kv-name">{row.label}</span>
              <span class="kv-count">{compact(row.count)}</span>
            {:else}
              <span class="kv-type t-{SHORT[row.info?.kind ?? ''] ?? 'other'}" data-tip={kindName(row.info?.kind ?? '')}>{SHORT[row.info?.kind ?? ''] ?? row.info?.kind}</span>
              <span class="kv-name mono" data-tip={row.path}>{row.label}</span>
              {#if row.info?.ttl_ms !== null && row.info?.ttl_ms !== undefined}<span class="kv-ttl" data-tip={m.kv_ttl()}>{shortTtl(row.info.ttl_ms, m.kv_day_short())}</span>{/if}
              <span class="kv-size">{row.info?.bytes ? bytes(row.info.bytes) : ''}</span>
            {/if}
          </div>
        {/each}
      </div>
    </div>

    <footer class="kv-foot">
      <span class="kv-loaded" role="status">{st.scanning && !st.started ? m.kv_scanning() : loadedLine}</span>
      <span class="spacer"></span>
      {#if st.scanning && st.started}
        <button class="btn ghost" type="button" onclick={() => st.stop()}>{m.kv_stop()}</button>
      {:else if !st.done && st.started}
        <button class="btn ghost" type="button" onclick={() => void st.more(sid)}>{m.kv_more()}</button>
        <button class="btn ghost" type="button" data-tip={m.kv_load_all_hint({ cap: num(LOAD_ALL_CAP) })} onclick={() => void st.all(sid)}>{m.kv_load_all()}</button>
      {:else if st.done}
        <span class="kv-done">{m.kv_scan_done()}</span>
      {/if}
    </footer>
  </div>

  <KeyDetail {st} {session} />
</div>

{#if creating}
  <KeyNew {st} {session} prefix={st.pattern.endsWith(':*') ? st.pattern.slice(0, -1) : ''} onclose={() => (creating = false)} />
{/if}
