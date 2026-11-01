<script lang="ts" generics="T extends string">
  import { play } from '../lib/sound';

  interface Props {
    label: string;
    value: T;
    options: readonly { value: T; label: string }[];
    onchange: (value: T) => void;
    class?: string;
  }
  let { label, value, options, onchange, class: extra = '' }: Props = $props();
  let buttons: HTMLButtonElement[] = $state([]);

  function pick(next: T) {
    if (next !== value) play('toggle');
    onchange(next);
  }

  function onkeydown(e: KeyboardEvent, i: number) {
    const step = { ArrowRight: 1, ArrowDown: 1, ArrowLeft: -1, ArrowUp: -1 }[e.key];
    if (!step) return;
    e.preventDefault();
    const next = (i + step + options.length) % options.length;
    const option = options[next];
    if (!option) return;
    pick(option.value);
    buttons[next]?.focus();
  }
</script>

<div class="seg {extra}" role="radiogroup" aria-label={label}>
  {#each options as option, i (option.value)}
    <button
      bind:this={buttons[i]}
      type="button"
      role="radio"
      aria-checked={option.value === value}
      tabindex={option.value === value ? 0 : -1}
      onclick={() => pick(option.value)}
      onkeydown={(e) => onkeydown(e, i)}>{option.label}</button
    >
  {/each}
</div>
