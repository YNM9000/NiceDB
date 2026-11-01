<script lang="ts">
  import type { FolderTint } from '../lib/bindings';
  let { tint, size = 14 }: { tint: FolderTint | null; size?: number } = $props();
</script>

<svg class="tint-dot" data-tint={tint ?? 'none'} width={size} height={size} viewBox="0 0 14 14" aria-hidden="true">
  <circle cx="7" cy="7" r="4.5" />
</svg>
