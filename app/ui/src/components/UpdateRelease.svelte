<script lang="ts">
  import { appearance } from '../lib/appearance.svelte';
  import { update } from '../lib/update.svelte';
  import { m } from '../paraglide/messages';

  let { compact = false, ondetails }: { compact?: boolean; ondetails?: () => void } = $props();

  const r = $derived(update.release);
  const notes = $derived(
    (r ? (appearance.value.locale === 'en' ? r.notes.en : r.notes.it) : '')
      .split('\n')
      .map((l) => l.replace(/^\s*[-*•]\s*/, '').trim())
      .filter(Boolean),
  );
  const pct = $derived(update.progress === null ? null : Math.round(update.progress * 100));
  const date = $derived(r?.pub_date ? new Intl.DateTimeFormat(appearance.value.locale, { dateStyle: 'long' }).format(new Date(r.pub_date)) : null);
</script>

{#if r}
  <div class="upd-release" class:compact data-phase={update.phase} data-testid="upd-release">
    <p class="upd-title">
      {m.upd_available()}<span class="upd-tag mono">{r.version}</span>
      {#if date && !compact}<span class="upd-date">{m.upd_published({ date })}</span>{/if}
    </p>

    {#if notes.length > 0 && !compact}
      <ul class="upd-notes" aria-label={m.upd_notes()}>
        {#each notes as n, i (i)}<li>{n}</li>{/each}
      </ul>
    {/if}

    {#if update.phase === 'downloading'}
      <div class="upd-progress">
        <progress max="100" value={pct ?? undefined}></progress>
        <span class="upd-status">{pct === null ? m.upd_downloading_unknown() : m.upd_downloading({ pct })}</span>
      </div>
    {:else if update.phase === 'installing'}
      <p class="upd-status">{m.upd_installing()}</p>
    {:else if update.phase === 'ready'}
      <p class="upd-status">{m.upd_ready()}</p>
    {:else if update.phase === 'saved' && update.outcome}
      <p class="upd-status upd-saved">{m.upd_saved({ path: update.outcome.path })}</p>
    {:else if update.phase === 'error'}
      <p class="upd-error" role="alert">{update.error}</p>
    {/if}

    <div class="upd-actions">
      {#if update.phase === 'available' || update.phase === 'error'}
        <button class="btn primary compact" type="button" onclick={() => void update.install()}>{m.upd_install()}</button>
      {:else if update.phase === 'ready'}
        <button class="btn primary compact" type="button" onclick={() => void update.restart()}>{m.upd_restart()}</button>
      {/if}
      {#if compact && ondetails}
        <button class="btn ghost compact" type="button" onclick={ondetails}>{m.upd_notes()}</button>
      {/if}
      {#if compact && !update.busy}
        <button class="btn ghost compact" type="button" onclick={() => update.later()}>{m.upd_later()}</button>
      {/if}
    </div>
  </div>
{/if}
