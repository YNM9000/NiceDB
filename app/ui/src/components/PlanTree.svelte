<script lang="ts">
  import { count, factor, hottest, label, ms, native, pct, shares, weights, type Comparison, type Plan } from '../lib/plan';
  import { m } from '../paraglide/messages';

  interface Props {
    plan: Plan;
    comparison: Comparison | null;
    selected: number | null;
    onselect: (id: number | null) => void;
    compact?: boolean;
  }
  let { plan, comparison, selected, onselect, compact = false }: Props = $props();

  const share = $derived(shares(plan));
  const weight = $derived(weights(plan));
  const measured = $derived(plan.analyzed);
  const hot = $derived(hottest(plan));
  const value = (v: number | null | undefined) => (measured ? ms(v) : count(v));
</script>

<section class="pt" class:compact>
  {#if compact}
    <p class="pt-cap"><b>{m.plan_nodes_title()}</b><span>{plan.nodes.length === 1 ? m.plan_nodes_one() : m.plan_nodes_count({ count: String(plan.nodes.length) })}</span></p>
  {/if}
  <table class="pt-table">
    <thead>
      <tr>
        <th class="pt-n">{m.plan_col_n()}</th>
        <th class="pt-op">{m.plan_col_op()}</th>
        {#if measured}<th class="num">{m.plan_col_rows()}</th>{/if}
        <th class="num">{m.plan_col_est()}</th>
        <th class="num">{measured ? m.plan_col_time() : m.plan_col_cost()}</th>
        {#if comparison}<th class="num">{m.plan_col_before()}</th>{/if}
        <th class="pt-share">{m.plan_col_share()}</th>
      </tr>
    </thead>
    <tbody>
      {#each plan.nodes as n (n.id)}
        {@const d = comparison?.nodes.get(n.id)}
        {@const worst = n.id === plan.worst_estimate}
        <tr class:hot={n.id === hot} class:worst class:sel={selected === n.id}>
          <td class="pt-n"><span class="pt-no">{n.id + 1}</span></td>
          <td class="pt-op">
            <div class="pt-tree" class:child={n.depth > 0} style:--depth={Math.min(n.depth, 14)}>
              <button type="button" class="pt-label" aria-pressed={selected === n.id} onclick={() => onselect(selected === n.id ? null : n.id)}>{label(n)}</button>
              <span class="pt-native">{native(n)}{#if n.condition ?? n.filter}<span class="pt-cond">{' · '}{(n.condition ?? n.filter ?? '').replace(/^\((.*)\)$/, '$1')}</span>{/if}</span>
            </div>
          </td>
          {#if measured}
            <td class="num">{count(n.rows)}{#if (n.loops ?? 1) > 1}<small class="pt-loops"> ×{count(n.loops)}</small>{/if}</td>
          {/if}
          <td class="num pt-est">{count(n.est_rows)}{#if worst}<small class="pt-off">×{factor(Math.max((n.est_rows ?? 0) / Math.max(1, n.rows ?? 1), (n.rows ?? 0) / Math.max(1, n.est_rows ?? 1)))}</small>{/if}</td>
          <td class="num pt-time">{value(weight[n.id])}</td>
          {#if comparison}
            <td class="num pt-before">{d?.before !== null && d?.before !== undefined ? value(d.before) : '—'}</td>
          {/if}
          <td class="pt-share">
            <span class="pt-bar"><i style:width="{Math.min(100, (share[n.id] ?? 0) * 100)}%"></i></span>
            <span class="pt-pct">{pct(share[n.id] ?? 0)}</span>
          </td>
        </tr>
      {/each}
    </tbody>
  </table>
</section>
