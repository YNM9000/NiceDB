<script lang="ts" generics="T extends string">
  import type { Snippet } from 'svelte';
  import { tick } from 'svelte';
  import { float } from '../lib/float';
  import { escapeTo, roveMenu } from '../lib/focus';

  interface Props {
    label: string;
    items: readonly { value: T; label: string }[];
    onpick: (value: T) => void;
    disabled?: boolean;
    children: Snippet;
  }
  let { label, items, onpick, disabled = false, children }: Props = $props();
  let open = $state(false);
  let root: HTMLDivElement;
  let button = $state<HTMLButtonElement>();
  let list = $state<HTMLDivElement>();

  function toggle() {
    open = !open;
    if (open) void tick().then(() => list?.querySelector<HTMLElement>('[role=menuitem]')?.focus());
  }

  $effect(() => {
    if (!open) return;
    const close = (e: Event) => {
      if (e instanceof KeyboardEvent ? e.key === 'Escape' : !root.contains(e.target as Node)) {
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

<div class="menu" bind:this={root}>
  <button bind:this={button} class="btn" type="button" aria-haspopup="menu" aria-expanded={open} aria-label={label} {disabled} onclick={toggle}>
    {@render children()}
  </button>
  {#if open && button}
    <div class="menu-list" role="menu" tabindex="-1" bind:this={list} onkeydown={(e) => list && roveMenu(e, list)} use:float={{ anchor: button, onaway: () => (open = false) }}>
      {#each items as item (item.value)}
        <button
          type="button"
          role="menuitem"
          tabindex="-1"
          onclick={() => {
            open = false;
            onpick(item.value);
          }}>{item.label}</button
        >
      {/each}
    </div>
  {/if}
</div>
