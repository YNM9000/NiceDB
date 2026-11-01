<script lang="ts">
  import { X } from '@lucide/svelte';
  import { keys } from '../lib/contextmenu.svelte';
  import { dismiss, pauseToast, resumeToast, toasts } from '../lib/toast.svelte';
  import { m } from '../paraglide/messages';

  const toast = $derived(toasts.current);

  async function act() {
    const run = toast?.action?.run;
    dismiss();
    await run?.();
  }
</script>

<div class="toast-region" role="status" aria-live={toast?.tone === 'error' ? 'assertive' : 'polite'} aria-atomic="true">
  {#if toast}
    {#key toast.id}
      <!-- svelte-ignore a11y_no_static_element_interactions -->
      <div class="toast" class:error={toast.tone === 'error'} onmouseenter={pauseToast} onmouseleave={resumeToast} onfocusin={pauseToast} onfocusout={resumeToast}>
        <span class="toast-text">{toast.text}</span>
        {#if toast.action}
          <span class="toast-dot" aria-hidden="true">·</span>
          <button class="toast-act" type="button" data-tip={keys('Mod+Z')} onclick={() => void act()}>{toast.action.label}</button>
        {/if}
        <button class="toast-x" type="button" aria-label={m.toast_close()} onclick={dismiss}><X size={13} strokeWidth={1.9} /></button>
      </div>
    {/key}
  {/if}
</div>
