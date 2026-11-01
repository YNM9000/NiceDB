<script lang="ts">
  import { buffersRead, count, ms, pct, segments, type Comparison, type Report } from '../lib/plan';
  import { m } from '../paraglide/messages';

  let { report, comparison, seq }: { report: Report; comparison: Comparison | null; seq: number } = $props();

  const plan = $derived(report.plan);
  const segs = $derived(segments(plan));
  const hot = $derived(segs.find((s) => s.hot) ?? null);
  const pages = $derived(buffersRead(plan));
  const root = $derived(plan.nodes[0]);
  const returned = $derived(root ? (root.rows ?? root.est_rows) : null);
  const estimated = $derived(!plan.analyzed);
  const stamp = $derived(Math.max(1, Math.ceil(seq / 2)));
</script>

<div class="pv-figures">
  <div class="pf">
    <span class="pf-k">{estimated ? m.plan_cost() : m.plan_total()}</span>
    <b class="pf-v">{estimated ? count(root?.total_cost) : ms(plan.total_ms)}</b>
    {#if comparison && comparison.before !== null}
      <span class="pf-was">{m.plan_compare_before()} {estimated ? count(comparison.before) : ms(comparison.before)}</span>
    {/if}
  </div>
  {#if plan.planning_ms !== null}
    <div class="pf">
      <span class="pf-k">{m.plan_planning()}</span>
      <b class="pf-v">{ms(plan.planning_ms)}</b>
    </div>
  {/if}
  {#if pages !== null}
    <div class="pf">
      <span class="pf-k">{m.plan_pages()}</span>
      <b class="pf-v">{count(pages)}</b>
    </div>
  {/if}
  <div class="pf">
    <span class="pf-k">{m.plan_returned()}</span>
    <b class="pf-v">{count(returned)}</b>
  </div>
  {#if segs.length}
    <div class="pf pf-split">
      <span class="pf-k">{estimated ? m.plan_split_cost() : m.plan_split()}</span>
      <div class="pf-bar" aria-hidden="true">
        {#each segs as s, i (i)}<i class:hot={s.hot} class:rest={s.id === null} style:flex-grow={Math.max(s.share, 0.015)}></i>{/each}
      </div>
      {#if hot}
        <span class="pf-legend"><em>{hot.label}: {pct(hot.share)}</em><span>{m.plan_other()} {pct(1 - hot.share)}</span></span>
      {/if}
    </div>
  {/if}
  <div class="pv-stamps" aria-hidden="true">
    <span class="pv-stamp"><small>{m.plan_stamp()}</small><b>{m.plan_stamp_n({ n: String(stamp).padStart(4, '0') })}</b></span>
    {#if report.execution.kind === 'estimated'}<span class="pv-stamp alt">{m.plan_stamp_estimate()}</span>{/if}
    {#if report.execution.kind === 'measured_rolled_back'}<span class="pv-stamp alt">{m.plan_stamp_rolled_back()}</span>{/if}
  </div>
</div>
