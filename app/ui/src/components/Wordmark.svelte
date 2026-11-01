<script lang="ts">
  import { wordmarkGeometry } from '../lib/wordmark';

  let { size = 18, text = false, class: cls = '' }: { size?: number; text?: boolean; class?: string } = $props();
  const g = $derived(wordmarkGeometry(size, text));
</script>

<svg class="wordmark {cls}" data-size={size} data-text={text ? '' : undefined} width={g.width} height={g.height} viewBox="0 0 {g.width} {g.height}" role="img" aria-label="NiceDB">
  <path d={g.d} transform={g.glyphs} fill="currentColor" />
  {#each g.rules as r, i (i)}<rect class="wordmark-rule" x={r.x} y={r.y} width={r.w} height={r.h} />{/each}
</svg>
