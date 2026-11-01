<script lang="ts">
  import { Check, ClipboardCopy, RotateCcw, TextCursorInput } from '@lucide/svelte';
  import { hasDryRun } from '../lib/design';
  import type { Design } from '../lib/design.svelte';
  import { m } from '../paraglide/messages';

  let { design }: { design: Design } = $props();
  const empty = $derived(design.script.statements.length === 0);
  const idle = $derived(empty || design.applying || design.trial || design.loading);
</script>

<button class="btn primary dz-apply" type="button" disabled={idle || design.blocked} data-tip={hasDryRun(design.engine) ? m.dz_apply_dry_title() : m.dz_apply_title()} onclick={() => void design.apply()}>
  <Check size={13} strokeWidth={1.8} aria-hidden="true" />
  {design.applying ? m.dz_applying() : design.origin ? m.dz_apply() : m.dz_create()}
</button>
<button class="btn" type="button" disabled={empty} aria-label={m.dz_copy()} data-tip={m.dz_copy()} onclick={() => void design.copy()}>
  <ClipboardCopy size={13} strokeWidth={1.8} aria-hidden="true" />
  {m.dz_copy()}
</button>
<button class="btn" type="button" disabled={empty} aria-label={m.tool_open_editor()} data-tip={m.tool_open_editor()} onclick={() => void design.openInEditor()}>
  <TextCursorInput size={13} strokeWidth={1.8} aria-hidden="true" />
  {m.tool_open_editor()}
</button>
<button class="btn" type="button" disabled={empty || design.applying || design.trial} aria-label={m.dz_reset()} data-tip={m.dz_reset_hint()} onclick={() => design.reset()}>
  <RotateCcw size={13} strokeWidth={1.8} aria-hidden="true" />
  {m.dz_reset()}
</button>
<span class="spacer"></span>
