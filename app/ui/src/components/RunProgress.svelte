<script lang="ts">
  import { compact } from '../lib/format';
  import { liveProgress, share } from '../lib/progress';
  import { bytes } from '../lib/tools';
  import type { Tab } from '../lib/workspace.svelte';
  import { m } from '../paraglide/messages';

  let { tab }: { tab: Tab } = $props();

  const live = $derived(tab.running ? liveProgress(tab.results) : null);
  const p = $derived(live?.progress ?? null);
  const part = $derived(p ? share(p) : null);
</script>

{#if p}
  <div class="run-progress" role="status" aria-live="polite">
    <span class="run-progress-bar" class:unknown={part === null} aria-hidden="true"><span style:inline-size={part === null ? null : `${(part * 100).toFixed(1)}%`}></span></span>
    <span class="run-progress-text">
      {#if p.total_rows}{m.run_progress_of({ read: compact(p.read_rows ?? 0), total: compact(p.total_rows) })}{:else}{m.run_progress_read({ read: compact(p.read_rows ?? 0) })}{/if}
      · {bytes(p.read_bytes)}{#if part !== null}&nbsp;· {Math.floor(part * 100)}%{/if}
    </span>
  </div>
{/if}
