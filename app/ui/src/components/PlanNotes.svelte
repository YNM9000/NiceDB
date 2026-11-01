<script lang="ts">
  import { Check, Copy, X } from '@lucide/svelte';
  import { count, factor, findingText, hottest, label, ms, native, pct, segments, type Comparison, type Report } from '../lib/plan';
  import { m } from '../paraglide/messages';

  interface Props {
    report: Report;
    previous: Report | null;
    comparison: Comparison | null;
    selected: number | null;
    onclose: () => void;
  }
  let { report, previous, comparison, selected, onclose }: Props = $props();
  let copied = $state<number | null>(null);

  const plan = $derived(report.plan);
  const hot = $derived(hottest(plan));
  const hotSeg = $derived(segments(plan).find((s) => s.hot) ?? null);
  const node = $derived(selected !== null ? (plan.nodes[selected] ?? null) : null);

  const summary = $derived.by(() => {
    const n = hot !== null ? plan.nodes[hot] : undefined;
    if (n && hotSeg && plan.analyzed) return m.plan_summary_measured({ total: ms(plan.total_ms), op: label(n), share: pct(hotSeg.share) });
    if (n && hotSeg) return m.plan_summary_estimated({ op: label(n), share: pct(hotSeg.share) });
    const steps = plan.nodes.filter((x) => x.relation).map((x) => native(x));
    return steps.length ? m.plan_summary_order({ steps: steps.join(' → ') }) : '';
  });

  const ENGINE_NAME: Partial<Record<Report['plan']['engine'], string>> = { sqlite: 'SQLite', clickhouse: 'ClickHouse', firebird: 'Firebird', hana: 'SAP HANA', db2: 'Db2' };

  const note = $derived.by(() => {
    const e = report.execution;
    if (e.kind === 'measured_rolled_back') return m.plan_rolled_back_note();
    if (e.kind !== 'estimated') return '';
    if (e.reason === 'engine_cannot_measure') return m.plan_reason_engine_cannot_measure({ engine: ENGINE_NAME[report.plan.engine] ?? report.plan.engine });
    return {
      requested: m.plan_reason_requested,
      production_write: m.plan_reason_production_write,
      no_safe_rollback: m.plan_reason_no_safe_rollback,
      engine_cannot_measure: () => '',
      no_savepoint: m.plan_reason_no_savepoint,
      no_privilege: m.plan_reason_no_privilege,
    }[e.reason]();
  });

  const verdict = $derived.by(() => {
    const r = comparison?.ratio ?? null;
    if (r === null) return '';
    if (r >= 1.15) return m.plan_compare_faster({ ratio: factor(r) });
    if (r <= 1 / 1.15) return m.plan_compare_slower({ ratio: factor(1 / r) });
    return m.plan_compare_same();
  });

  const SEVERITY = { high: m.plan_severity_high, medium: m.plan_severity_medium, low: m.plan_severity_low };

  async function copy(i: number, sql: string) {
    await navigator.clipboard.writeText(sql);
    copied = i;
    setTimeout(() => {
      if (copied === i) copied = null;
    }, 1600);
  }

  const val = (v: number | null | undefined) => (plan.analyzed ? ms(v) : count(v));
</script>

<aside class="pv-notes" aria-label={m.plan_notes()}>
  <h4 class="pn-head"><span>{m.plan_notes()}</span><small>{report.findings.length === 1 ? m.plan_notes_one() : m.plan_notes_count({ count: String(report.findings.length) })}</small></h4>

  {#if node}
    <section class="pn-node">
      <header>
        <span class="pn-node-n">{node.id + 1}</span>
        <b>{label(node)}</b>
        <button class="icon-btn" type="button" aria-label={m.plan_close_node()} onclick={onclose}><X size={13} strokeWidth={2} /></button>
      </header>
      <p class="pn-node-native">{native(node)}</p>
      <dl>
        {#if plan.analyzed}<dt>{m.plan_col_rows()}</dt><dd>{count(node.rows)}{#if (node.loops ?? 1) > 1} × {count(node.loops)}{/if}</dd>{/if}
        <dt>{m.plan_col_est()}</dt><dd>{count(node.est_rows)}</dd>
        <dt>{plan.analyzed ? m.plan_col_time() : m.plan_col_cost()}</dt><dd>{val(plan.analyzed ? node.self_ms : node.total_cost)}</dd>
        {#if node.condition}<dt>{m.plan_cond()}</dt><dd class="code">{node.condition}</dd>{/if}
        {#if node.filter}<dt>{m.plan_filter()}</dt><dd class="code">{node.filter}</dd>{/if}
        {#each node.details.filter((d) => d.key !== 'Parent Relationship') as d (d.key)}<dt>{d.key}</dt><dd class="code">{d.value}</dd>{/each}
      </dl>
    </section>
  {/if}

  <section class="pn-what">
    <h5>{m.plan_what()}</h5>
    {#if summary}<p>{summary}</p>{/if}
    {#if note}<p class="pn-exec">{note}</p>{/if}
  </section>

  {#if comparison && previous && comparison.before !== null && comparison.after !== null}
    {@const max = Math.max(comparison.before, comparison.after)}
    <section class="pn-compare">
      <h5>{m.plan_compare_title()}</h5>
      <div class="pc-row"><span>{m.plan_compare_before()}</span><span class="pc-bar"><i style:width="{(comparison.before / max) * 100}%"></i></span><b>{val(comparison.before)}</b></div>
      <div class="pc-row now"><span>{m.plan_compare_now()}</span><span class="pc-bar"><i style:width="{(comparison.after / max) * 100}%"></i></span><b>{val(comparison.after)}</b></div>
      <p class="pc-verdict">{verdict}</p>
      <p class="pc-hint">{m.plan_compare_hint()}</p>
    </section>
  {/if}

  {#each report.findings as f, i (i)}
    {@const t = findingText(f)}
    <article class="pn-f sev-{f.severity}">
      <span class="pn-no" data-tip={SEVERITY[f.severity]()}>{i + 1}</span>
      <div class="pn-body">
        <h5>{t.title}</h5>
        <p>{t.body}</p>
        {#if f.suggestion}
          <div class="pn-sug">
            <p class="pn-sug-h"><span>{m.plan_suggestion()}</span><span>{SEVERITY[f.severity]()}</span></p>
            <pre>{f.suggestion}</pre>
            <div class="pn-sug-f">
              <button class="btn" type="button" onclick={() => f.suggestion && void copy(i, f.suggestion)}>
                {#if copied === i}<Check size={13} strokeWidth={2} aria-hidden="true" />{m.plan_copied()}{:else}<Copy size={13} strokeWidth={1.8} aria-hidden="true" />{m.plan_copy()}{/if}
              </button>
              <span>{m.plan_try_note()}</span>
            </div>
          </div>
        {/if}
      </div>
    </article>
  {:else}
    <p class="pn-none">{m.plan_none()}</p>
  {/each}
</aside>
