<script lang="ts">
  import { sqlOf } from '../lib/family';
  import { ClipboardCopy, RefreshCw, TextCursorInput } from '@lucide/svelte';
  import { api, type Sizes } from '../lib/api';
  import { copyText } from '../lib/contextmenu.svelte';
  import { compact, num } from '../lib/format';
  import { bytes, dropIndexSql, flavorOf } from '../lib/tools';
  import { errorText } from '../lib/tools.svelte';
  import { newTab, type sessionFor, type Tab } from '../lib/workspace.svelte';
  import { m } from '../paraglide/messages';
  import PartitionList from './PartitionList.svelte';

  let { tab, session }: { tab: Tab; session: NonNullable<ReturnType<typeof sessionFor>> } = $props();

  let data = $state.raw<Sizes | null>(null);
  let failure = $state<string | null>(null);
  let onlyUnused = $state(false);

  async function load() {
    try {
      data = await api.serverSizes(session.info.session_id);
      failure = null;
    } catch (e) {
      failure = errorText(e);
    }
  }

  $effect(() => {
    void load();
  });

  const dialect = $derived(sqlOf(session.info));
  const db = $derived(session.schema?.current_database ?? null);
  const largest = $derived(Math.max(1, ...(data?.tables ?? []).map((t) => t.total_bytes ?? t.data_bytes ?? 0)));
  const total = $derived((data?.tables ?? []).reduce((a, t) => a + (t.total_bytes ?? t.data_bytes ?? 0), 0));
  const unused = $derived(data?.indexes.filter((i) => i.unused) ?? []);
  const indexes = $derived(onlyUnused ? unused : (data?.indexes ?? []));
  const wasted = $derived(unused.reduce((a, i) => a + (i.bytes ?? 0), 0));

  const flavor = $derived(flavorOf(dialect, session.info.server.product));
  const dropSql = (ns: string, name: string, table: string) => dropIndexSql(dialect, flavor, ns, name, table, db);
  const dropAll = $derived(unused.map((i) => dropSql(i.namespace, i.name, i.table)).join('\n'));
</script>

<header class="tool-head">
  <div class="tool-title">
    <h2 class="display">{m.tool_space()}</h2>
    {#if data}<p class="hint">{m.tool_space_summary({ tables: num(data.tables.length), size: bytes(total) })}</p>{/if}
  </div>
  <span class="spacer"></span>
  <button class="icon-btn" type="button" aria-label={m.schema_refresh()} onclick={() => void load()}><RefreshCw size={13} strokeWidth={1.8} /></button>
</header>

{#if failure && !data}
  <p class="tool-empty err">{failure}</p>
{:else if data}
  <div class="tool-scroll">
    <table class="tool-table sizes">
      <thead>
        <tr>
          <th>{m.tool_col_table()}</th>
          <th class="num">{m.tool_col_rows()}</th>
          <th class="num">{m.tool_col_data()}</th>
          <th class="num">{m.tool_col_indexes()}</th>
          <th class="num">{m.tool_col_total()}</th>
          <th class="bar-col"><span class="tool-sr">{m.tool_col_share()}</span></th>
        </tr>
      </thead>
      <tbody>
        {#each data.tables as t (`${t.namespace}.${t.name}`)}
          {@const size = t.total_bytes ?? t.data_bytes ?? 0}
          <tr>
            <td>{t.name}{#if (session.schema?.namespaces.length ?? 1) > 1}<span class="sub">{t.namespace}</span>{/if}</td>
            <td class="num">{t.rows === null ? '—' : compact(t.rows)}</td>
            <td class="num">{bytes(t.data_bytes)}</td>
            <td class="num">{bytes(t.index_bytes)}</td>
            <td class="num strong">{bytes(t.total_bytes ?? t.data_bytes)}</td>
            <td class="bar-col"><span class="tool-bar"><i style:width={`${Math.max(1.5, (size / largest) * 100)}%`}></i></span></td>
          </tr>
        {/each}
      </tbody>
    </table>

    {#if data.hypertables?.length}
      {@const saved = data.hypertables.reduce((a, h) => a + Math.max(0, (h.before_compression ?? 0) - (h.after_compression ?? 0)), 0)}
      <div class="tool-subhead">
        <h3 class="caps tool-sub">{m.tool_hypertables()}</h3>
        <span class="hint">{saved > 0 ? m.tool_hyper_saved({ count: num(data.hypertables.length), size: bytes(saved) }) : m.tool_hyper_count({ count: num(data.hypertables.length) })}</span>
      </div>
      <table class="tool-table hypertables">
        <thead>
          <tr>
            <th>{m.tool_col_table()}</th>
            <th class="num">{m.tool_col_chunks()}</th>
            <th class="num">{m.tool_col_compressed()}</th>
            <th class="num">{m.tool_col_before()}</th>
            <th class="num">{m.tool_col_after()}</th>
            <th class="num">{m.tool_col_total()}</th>
          </tr>
        </thead>
        <tbody>
          {#each data.hypertables as h (`${h.namespace}.${h.name}`)}
            {@const ratio = h.before_compression && h.after_compression ? h.before_compression / h.after_compression : null}
            <tr>
              <td>{h.name}{#if (session.schema?.namespaces.length ?? 1) > 1}<span class="sub">{h.namespace}</span>{/if}</td>
              <td class="num">{h.chunks === null ? '—' : num(h.chunks)}</td>
              <td class="num">{h.compressed_chunks === null ? '—' : num(h.compressed_chunks)}</td>
              <td class="num">{bytes(h.before_compression)}</td>
              <td class="num">{bytes(h.after_compression)}{#if ratio && ratio >= 1.05}<span class="sub">{m.tool_hyper_ratio({ ratio: num(Math.round(ratio * 10) / 10) })}</span>{/if}</td>
              <td class="num strong">{bytes(h.total_bytes)}</td>
            </tr>
          {/each}
        </tbody>
      </table>
    {/if}

    {#if data.partitions?.length}<PartitionList partitions={data.partitions} showNamespace={new Set(data.partitions.map((p) => p.namespace)).size > 1} chunks={!!data.hypertables?.length} />{/if}

    <div class="tool-subhead">
      <h3 class="caps tool-sub">{m.tool_indexes()}</h3>
      {#if data.usage_known}
        <span class="hint">{unused.length ? m.tool_unused_summary({ count: num(unused.length), size: bytes(wasted) }) : m.tool_unused_none()}</span>
        <span class="spacer"></span>
        <label class="check"><input type="checkbox" bind:checked={onlyUnused} disabled={!unused.length} /> {m.tool_only_unused()}</label>
        {#if unused.length}
          <button class="btn ghost" type="button" onclick={() => copyText(dropAll)}><ClipboardCopy size={13} strokeWidth={1.8} aria-hidden="true" /> {m.tool_copy_drops()}</button>
          <button class="btn ghost" type="button" onclick={() => newTab(tab.connectionId, dropAll)}><TextCursorInput size={13} strokeWidth={1.8} aria-hidden="true" /> {m.tool_open_editor()}</button>
        {/if}
      {:else}
        <span class="hint">{m.tool_usage_unknown()}</span>
      {/if}
    </div>
    <table class="tool-table indexes">
      <thead>
        <tr>
          <th>{m.tool_col_index()}</th>
          <th>{m.tool_col_table()}</th>
          <th class="num">{m.tool_col_size()}</th>
          <th class="num">{m.tool_col_scans()}</th>
          <th><span class="tool-sr">{m.tool_col_state()}</span></th>
        </tr>
      </thead>
      <tbody>
        {#each indexes as i (`${i.namespace}.${i.table}.${i.name}`)}
          <tr class:unused={i.unused}>
            <td class="mono">{i.name}</td>
            <td>{i.table}</td>
            <td class="num">{bytes(i.bytes)}</td>
            <td class="num">{i.scans === null ? '—' : compact(i.scans)}</td>
            <td>
              {#if i.unused}<span class="tool-tag warn">{m.tool_never_used()}</span>{:else if i.unique}<span class="tool-tag">{m.tool_unique()}</span>{/if}
            </td>
          </tr>
        {:else}
          <tr><td colspan="5" class="tool-none">{m.tool_no_indexes()}</td></tr>
        {/each}
      </tbody>
    </table>
    {#if data.usage_known}<p class="hint tool-foot">{m.tool_usage_since()}</p>{/if}
  </div>
{:else}
  <p class="tool-empty">{m.schema_loading()}</p>
{/if}
