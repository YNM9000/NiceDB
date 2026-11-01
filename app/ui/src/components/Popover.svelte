<script lang="ts">
  import type { Snippet } from 'svelte';
  import { tick } from 'svelte';
  import { float } from '../lib/float';
  import { escapeTo, focusFirst } from '../lib/focus';

  interface Props {
    label: string;
    open: boolean;
    class?: string;
    disabled?: boolean;
    span?: string;
    trigger: Snippet;
    children: Snippet;
  }
  let { label, open = $bindable(), class: cls = 'btn', disabled = false, span, trigger, children }: Props = $props();
  let root = $state<HTMLDivElement>();
  let button = $state<HTMLButtonElement>();
  let panel = $state<HTMLDivElement>();

  $effect(() => {
    if (!open) return;
    void tick().then(() => focusFirst(panel));
    const close = (e: Event) => {
      if (e instanceof KeyboardEvent ? e.key === 'Escape' : !root?.contains(e.target as Node)) {
        if (e instanceof KeyboardEvent) escapeTo(root, button);
        open = false;
      }
    };
    window.addEventListener('pointerdown', close);
    window.addEventListener('keydown', close);
    return () => {
      window.removeEventListener('pointerdown', close);
      window.removeEventListener('keydown', close);
    };
  });
</script>

<div class="pop" bind:this={root}>
  <button bind:this={button} class={cls} type="button" aria-haspopup="dialog" aria-expanded={open} aria-label={label} data-tip={label} {disabled} onclick={() => (open = !open)}>
    {@render trigger()}
  </button>
  {#if open && button}
    <div class="pop-panel" role="dialog" bind:this={panel} aria-label={label} use:float={{ anchor: (span && button.closest<HTMLElement>(span)) || button, match: !!span, onaway: () => (open = false) }}>
      {@render children()}
    </div>
  {/if}
</div>
