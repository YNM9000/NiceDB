<script lang="ts">
  import { xmlParts } from '../lib/value';
  import { num } from '../lib/format';
  import { m } from '../paraglide/messages';

  const STEP = 2000;
  const CHARS = 100_000;

  let { text, xml = false }: { text: string; xml?: boolean } = $props();
  let limit = $state(STEP);
  let chars = $state(CHARS);

  const body = $derived(text.length > chars ? text.slice(0, chars) : text);
  const lines = $derived(body.split('\n'));
  const shownLines = $derived(lines.length > limit ? lines.slice(0, limit) : lines);
  const digits = $derived(String(lines.length).length);

  $effect(() => {
    void text;
    limit = STEP;
    chars = CHARS;
  });
</script>

<div class="vt" class:xml style:--vt-digits={digits}>
  <ol class="vt-lines">
    {#each shownLines as line, i (i)}
      <li><span class="vt-n" aria-hidden="true">{i + 1}</span><span class="vt-l">{#if xml}{#each xmlParts(line) as p, k (k)}<span class="x-{p.cls}">{p.text}</span>{/each}{:else}{line}{/if}{#if !line}&#8203;{/if}</span></li>
    {/each}
  </ol>
  {#if lines.length > limit}
    <button class="btn ghost vt-more" type="button" onclick={() => (limit += STEP * 5)}>{m.vp_more_lines({ count: num(lines.length - limit) })}</button>
  {:else if text.length > chars}
    <button class="btn ghost vt-more" type="button" onclick={() => (chars += CHARS * 5)}>{m.vp_more_chars({ count: num(text.length - chars) })}</button>
  {/if}
</div>
