<script lang="ts">
  import { onDestroy, untrack } from 'svelte';
  import type { NoteScope } from '../lib/api';
  import { noteOf, saveNote } from '../lib/library.svelte';
  import { renderMarkdown } from '../lib/markdown';
  import { m } from '../paraglide/messages';

  interface Props {
    scope: NoteScope;
    id: string;
    title: string;
    mode: 'write' | 'preview';
  }
  let { scope, id, title, mode }: Props = $props();

  const SAVE_MS = 600;
  const start = untrack(() => noteOf(scope, id)?.body ?? '');
  let text = $state(start);
  let saved = start;
  let status = $state<'idle' | 'dirty' | 'saving' | 'failed'>('idle');
  let timer: ReturnType<typeof setTimeout> | undefined;

  async function flush() {
    clearTimeout(timer);
    if (text === saved) return;
    const body = text;
    status = 'saving';
    try {
      await saveNote(scope, id, body);
      saved = body;
      status = text === body ? 'idle' : 'dirty';
    } catch {
      status = 'failed';
    }
  }

  function input() {
    status = 'dirty';
    clearTimeout(timer);
    timer = setTimeout(() => void flush(), SAVE_MS);
  }

  onDestroy(() => void flush());

  const html = $derived(mode === 'preview' ? renderMarkdown(text) : '');

  function noLinks(e: MouseEvent) {
    if ((e.target as Element | null)?.closest('a')) e.preventDefault();
  }
</script>

<div class="note">
  <div class="note-head">
    <h3 class="note-title">{title}</h3>
    <span class="note-state" role="status" class:err={status === 'failed'}>
      {#if status === 'failed'}{m.panel_notes_failed()}{:else if status === 'idle' && text}{m.panel_notes_saved()}{:else if status !== 'idle'}{m.panel_notes_saving()}{/if}
    </span>
  </div>
  {#if mode === 'write'}
    <textarea class="note-text" aria-label={title} placeholder={m.panel_notes_placeholder()} spellcheck="false" bind:value={text} oninput={input} onblur={() => void flush()}></textarea>
  {:else}
    <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
    <div class="note-md md" onclick={noLinks} onauxclick={noLinks}>
      {#if text.trim()}{@html html}{:else}<p class="hint">{m.panel_notes_empty()}</p>{/if}
    </div>
  {/if}
</div>
