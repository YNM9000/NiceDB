<script lang="ts">
  import type { LongOp } from '../lib/bindings';
  import { percent } from '../lib/monitor';
  import { duration } from '../lib/tools';
  import { m } from '../paraglide/messages';

  let { ops }: { ops: readonly LongOp[] } = $props();
</script>

{#if ops.length}
  <table class="tool-table mon-merges">
    <thead>
      <tr>
        <th class="num">{m.tool_col_pid()}</th>
        <th>{m.mon_col_operation()}</th>
        <th class="num">{m.mon_col_running()}</th>
        <th class="num">{m.mon_col_left()}</th>
        <th class="bar-col"><span class="tool-sr">{m.mon_col_done()}</span></th>
      </tr>
    </thead>
    <tbody>
      {#each ops as o, i (`${o.session}.${i}`)}
        <tr>
          <td class="num mono">{o.session}</td>
          <td>{o.operation}{#if o.target}<span class="sub">{o.target}</span>{/if}</td>
          <td class="num">{duration(o.elapsed_ms)}</td>
          <td class="num">{duration(o.remaining_ms)}</td>
          <td class="bar-col" data-tip={percent(o.progress ?? null)}><span class="tool-bar"><i style:width={`${Math.max(1.5, (o.progress ?? 0) * 100)}%`}></i></span></td>
        </tr>
      {/each}
    </tbody>
  </table>
{:else}
  <p class="mon-calm">{m.mon_no_long_ops()}</p>
{/if}
