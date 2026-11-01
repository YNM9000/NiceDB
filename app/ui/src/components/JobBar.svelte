<script lang="ts">
  import { Square } from '@lucide/svelte';
  import { num } from '../lib/format';
  import { bytes, duration, fraction } from '../lib/tools';
  import type { Job } from '../lib/tools.svelte';
  import { m } from '../paraglide/messages';

  let { job, label }: { job: Job; label: string } = $props();

  let now = $state(Date.now());
  $effect(() => {
    const t = setInterval(() => (now = Date.now()), 500);
    return () => clearInterval(t);
  });

  const p = $derived(job.progress);
  const share = $derived(fraction(p));
  const amount = $derived.by(() => {
    if (!p) return m.tool_job_starting();
    const done = p.done ?? 0;
    switch (p.unit) {
      case 'bytes':
        return p.total ? m.tool_job_of({ done: bytes(done), total: bytes(p.total) }) : bytes(done);
      case 'tables':
        return p.total ? m.tool_job_tables_of({ done: num(done), total: num(p.total) }) : m.tool_job_tables({ done: num(done) });
      case 'pages':
        return m.tool_job_pages_of({ done: num(done), total: num(p.total ?? done) });
      default:
        return p.total ? m.tool_job_rows_of({ done: num(done), total: num(p.total) }) : m.tool_job_rows({ done: num(done) });
    }
  });
  const extra = $derived(p && p.unit !== 'rows' && (p.rows ?? 0) > 0 ? m.tool_job_rows({ done: num(p.rows ?? 0) }) : p && p.unit === 'tables' && (p.bytes ?? 0) > 0 ? bytes(p.bytes) : null);
</script>

<div class="job" role="status" aria-live="polite">
  <div class="job-line">
    <span class="job-label">{label}</span>
    <span class="job-amount">{amount}{#if extra}&nbsp;· {extra}{/if}&nbsp;· {duration(now - job.started)}</span>
    <span class="spacer"></span>
    <button class="btn danger job-stop" type="button" onclick={() => void job.cancel()}>
      <Square size={12} strokeWidth={2} aria-hidden="true" />
      {m.tool_job_cancel()}
    </button>
  </div>
  <div class="job-track" class:indeterminate={share === null} role="progressbar" aria-label={label} aria-valuemin={0} aria-valuemax={100} aria-valuenow={share === null ? undefined : Math.round(share * 100)}>
    <span class="job-fill" style:width={share === null ? undefined : `${(share * 100).toFixed(1)}%`}></span>
  </div>
</div>
