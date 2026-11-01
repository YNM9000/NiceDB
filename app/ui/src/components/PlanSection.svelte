<script lang="ts">
  import { count, ms, pct, segments, type Plan } from '../lib/plan';
  import { m } from '../paraglide/messages';

  let { plan }: { plan: Plan } = $props();

  const segs = $derived(segments(plan, 1));
  const hot = $derived(segs.find((s) => s.hot) ?? null);
  const rest = $derived(segs.find((s) => s.id === null) ?? null);
  const total = $derived(segs.reduce((a, s) => a + s.value, 0));
  const fmt = (v: number) => (plan.analyzed ? ms(v) : count(v));
</script>

{#if hot && total > 0}
  <div class="pv-section" aria-hidden="true">
    <p class="ps-cap">{plan.analyzed ? m.plan_section() : m.plan_section_cost()}</p>
    <div class="ps-dim ps-total"><span>{fmt(plan.total_ms ?? total)}</span></div>
    <div class="ps-chain">
      <div class="ps-dim hot" style:flex-grow={Math.max(hot.share, 0.08)}><span>{fmt(hot.value)} · {pct(hot.share)}</span></div>
      {#if rest}<div class="ps-dim" style:flex-grow={Math.max(rest.share, 0.08)}><span>{fmt(rest.value)}</span></div>{/if}
    </div>
    <div class="ps-bar">
      <i class="hot" style:flex-grow={Math.max(hot.share, 0.08)}></i>
      {#if rest}<i style:flex-grow={Math.max(rest.share, 0.08)}></i>{/if}
    </div>
    <div class="ps-labels">
      <span style:flex-grow={Math.max(hot.share, 0.08)}>{m.plan_col_n()} {(hot.id ?? 0) + 1} · {hot.label}</span>
      {#if rest}<span style:flex-grow={Math.max(rest.share, 0.08)}>{plan.nodes.length - 1 === 1 ? m.plan_nodes_one() : m.plan_nodes_count({ count: String(plan.nodes.length - 1) })}</span>{/if}
    </div>
  </div>
{/if}
