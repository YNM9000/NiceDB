<script lang="ts">
  import type { BackgroundMerge_Serialize as BackgroundMerge } from '../lib/bindings';
  import { num } from '../lib/format';
  import { percent } from '../lib/monitor';
  import { bytes, duration } from '../lib/tools';
  import { m } from '../paraglide/messages';

  let { merges }: { merges: readonly BackgroundMerge[] } = $props();
</script>

{#if merges.length}
  <table class="tool-table mon-merges">
    <thead>
      <tr>
        <th>{m.tool_col_table()}</th>
        <th>{m.mon_col_kind()}</th>
        <th class="num">{m.mon_col_parts()}</th>
        <th class="num">{m.tool_col_size()}</th>
        <th class="num">{m.mon_col_running()}</th>
        <th class="bar-col"><span class="tool-sr">{m.mon_col_done()}</span></th>
      </tr>
    </thead>
    <tbody>
      {#each merges as g, i (`${g.namespace}.${g.table}.${i}`)}
        <tr>
          <td>{g.table}<span class="sub">{g.namespace}</span></td>
          <td>{g.mutation ? m.mon_mutation() : m.mon_merge()}</td>
          <td class="num">{g.parts === null ? '—' : num(g.parts)}</td>
          <td class="num">{bytes(g.bytes)}</td>
          <td class="num">{duration(g.elapsed_ms)}</td>
          <td class="bar-col" data-tip={percent(g.progress)}><span class="tool-bar"><i style:width={`${Math.max(1.5, (g.progress ?? 0) * 100)}%`}></i></span></td>
        </tr>
      {/each}
    </tbody>
  </table>
{:else}
  <p class="mon-calm">{m.mon_no_merges()}</p>
{/if}
