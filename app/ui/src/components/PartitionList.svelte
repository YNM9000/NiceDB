<script lang="ts">
  import type { PartitionSize_Serialize as PartitionSize } from '../lib/bindings';
  import { compact, num } from '../lib/format';
  import { bytes } from '../lib/tools';
  import { m } from '../paraglide/messages';

  let { partitions, showNamespace, chunks = false }: { partitions: readonly PartitionSize[]; showNamespace: boolean; chunks?: boolean } = $props();

  const rowsKnown = $derived(partitions.some((p) => p.rows !== null));
  const largest = $derived(Math.max(1, ...partitions.map((p) => p.bytes ?? 0)));
</script>

<div class="tool-subhead">
  <h3 class="caps tool-sub">{chunks ? m.tool_chunks() : m.tool_partitions()}</h3>
  <span class="hint"
    >{chunks
      ? m.tool_chunks_summary({ count: num(partitions.length), compressed: num(partitions.filter((p) => p.compressed).length) })
      : m.tool_partitions_summary({ count: num(partitions.length), parts: num(partitions.reduce((a, p) => a + (p.parts ?? 0), 0)) })}</span
  >
</div>
<table class="tool-table partitions">
  <thead>
    <tr>
      <th>{m.tool_col_table()}</th>
      <th>{chunks ? m.tool_col_range() : m.tool_col_partition()}</th>
      {#if !chunks}<th class="num">{m.mon_col_parts()}</th>{/if}
      {#if rowsKnown}<th class="num">{m.tool_col_rows()}</th>{/if}
      <th class="num">{m.tool_col_on_disk()}</th>
      <th class="bar-col"><span class="tool-sr">{m.tool_col_share()}</span></th>
    </tr>
  </thead>
  <tbody>
    {#each partitions as p, i (`${p.namespace}.${p.table}.${p.partition}`)}
      {@const first = i === 0 || partitions[i - 1]?.table !== p.table || partitions[i - 1]?.namespace !== p.namespace}
      <tr class:group-start={first && i > 0}>
        <td>{#if first}{p.table}{#if showNamespace}<span class="sub">{p.namespace}</span>{/if}{/if}</td>
        <td class="mono">{p.partition}{#if p.compressed}<span class="tool-tag chunk-tag">{m.tool_chunk_compressed()}</span>{/if}</td>
        {#if !chunks}<td class="num" class:warn={(p.parts ?? 0) > 100}>{p.parts === null ? '—' : num(p.parts)}</td>{/if}
        {#if rowsKnown}<td class="num">{p.rows === null ? '—' : compact(p.rows)}</td>{/if}
        <td class="num">{bytes(p.bytes)}</td>
        <td class="bar-col"><span class="tool-bar"><i style:width={`${Math.max(1.5, ((p.bytes ?? 0) / largest) * 100)}%`}></i></span></td>
      </tr>
    {/each}
  </tbody>
</table>
