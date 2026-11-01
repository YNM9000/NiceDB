<script lang="ts">
  import { BookOpenCheck, Lock, LockOpen } from '@lucide/svelte';
  import { alwaysOpen, clock, guard, lock, refreshAccess, remaining, unlock, writesOpen } from '../lib/guard.svelte';
  import { prefs } from '../lib/prefs.svelte';
  import { m } from '../paraglide/messages';
  import { tick } from 'svelte';
  import { escapeTo, focusFirst } from '../lib/focus';

  let { sessionId }: { sessionId: string } = $props();
  let asking = $state(false);
  let busy = $state(false);
  let error = $state<string | null>(null);
  let root = $state<HTMLDivElement>();
  let key = $state<HTMLButtonElement>();
  let pop = $state<HTMLDivElement>();

  const left = $derived(remaining(sessionId));
  const open = $derived(writesOpen(sessionId));
  const always = $derived(alwaysOpen(sessionId));
  const safety = $derived(prefs.value.safety);
  const hint = $derived(
    `${safety.unlock_mode === 'session' ? m.guard_hint_session() : m.guard_hint_timed({ minutes: safety.unlock_minutes })} ${{ typed: m.guard_hint_typed, modal: m.guard_hint_modal, none: m.guard_hint_none }[safety.confirm_mode]()}`,
  );
  const closing = $derived(!open && !!guard.access[sessionId]?.closing);
  const note = $derived(guard.access[sessionId]?.note ?? null);

  $effect(() => {
    void refreshAccess(sessionId).catch(() => {});
  });

  $effect(() => {
    if (!asking) return;
    void tick().then(() => focusFirst(pop));
    const close = (e: Event) => {
      if (e instanceof KeyboardEvent ? e.key === 'Escape' : !root?.contains(e.target as Node)) {
        if (e instanceof KeyboardEvent) escapeTo(root, key);
        asking = false;
      }
    };
    window.addEventListener('pointerdown', close);
    window.addEventListener('keydown', close);
    return () => {
      window.removeEventListener('pointerdown', close);
      window.removeEventListener('keydown', close);
    };
  });

  async function act(fn: (id: string) => Promise<void>) {
    busy = true;
    error = null;
    try {
      await fn(sessionId);
      asking = false;
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
    } finally {
      busy = false;
    }
  }
</script>

<div class="wlock" class:open class:closing class:always bind:this={root}>
  <span class="wlock-stamp">{m.env_production()}</span>
  {#if always}
    <span class="wlock-always" role="status"><BookOpenCheck size={13} strokeWidth={1.8} aria-hidden="true" /><span class="wlock-always-text">{m.guard_always()}</span></span>
  {:else}
  <button
    class="wlock-key"
    type="button"
    bind:this={key}
    aria-haspopup={open ? undefined : 'dialog'}
    aria-expanded={open ? undefined : asking}
    aria-pressed={open}
    aria-label={open ? m.guard_lock() : m.guard_unlock()}
    disabled={busy}
    onclick={() => (open ? void act(lock) : (asking = !asking))}
  >
    <span class="wlock-side wlock-read">{m.guard_key_read()}</span>
    <span class="wlock-knob">
      {#if open}<LockOpen size={13} strokeWidth={2} aria-hidden="true" />{:else}<Lock size={13} strokeWidth={2} aria-hidden="true" />{/if}
    </span>
    <span class="wlock-side wlock-write">{m.guard_key_write()}</span>
    <span class="wlock-lamp" aria-hidden="true"></span>
    <span class="wlock-state">{open ? m.guard_open() : m.guard_locked()}</span>
  </button>
  {/if}
  {#if open && left > 0}
    <span class="wlock-time" role="timer">{m.guard_left({ time: clock(left) })}</span>
  {:else if closing}
    <span class="wlock-bubble wlock-closing" role="status">{m.guard_closing()}</span>
  {:else if note}
    <span class="wlock-bubble wlock-note" role="status">{note}</span>
  {/if}
  {#if asking}
    <div class="wlock-pop" role="dialog" bind:this={pop} aria-label={m.guard_unlock()}>
      <p>{hint}</p>
      {#if error}<p class="wlock-err" role="alert">{error}</p>{/if}
      <button class="btn danger" type="button" disabled={busy} onclick={() => void act(unlock)}>
        <LockOpen size={13} strokeWidth={1.8} aria-hidden="true" />
        {m.guard_unlock()}
      </button>
    </div>
  {/if}
</div>
