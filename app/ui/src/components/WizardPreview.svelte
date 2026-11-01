<script lang="ts">
  import { Database, Play, Settings, Table2 } from '@lucide/svelte';
  import { appearance, resolvedTheme } from '../lib/appearance.svelte';
  import { prefs } from '../lib/prefs.svelte';
  import type { Step } from '../lib/wizard';
  import { m } from '../paraglide/messages';

  let { step, unlock, confirm }: { step: Step; unlock: string; confirm: string } = $props();

  const a = $derived(appearance.value);
  const theme = $derived(resolvedTheme(a, appearance.systemDark));
  const rows = [
    [74, 70, 86],
    [52, 52, 72],
    [66, 52, 72],
    [58, 52, 72],
    [40, 52, 72],
    [80, 52, 72],
    [70, 52, 72],
    [62, 52, 72],
  ];
</script>

<figure class="wz-preview" aria-hidden="true">
  <figcaption class="caps">{m.wizard_preview()}</figcaption>
  <span class="wzp" data-style={a.style} data-palette={a.palette} data-theme={theme} style:--wz-code="{prefs.value.editor.font_size}px">
    <span class="wzp-bar">
      <span class="wzp-mark">Nice<i>DB</i></span>
      <span class="wzp-title">{m.nav_connections()}</span>
      <Settings size={10} strokeWidth={1.8} />
    </span>
    <span class="wzp-side">
      <span class="wzp-caps">{m.nav_connections()}</span>
      <span class="wzp-row"><Database size={9} strokeWidth={1.8} />{m.wizard_preview_database()}</span>
      <span class="wzp-row on"><Table2 size={9} strokeWidth={1.8} />{m.wizard_preview_orders()}</span>
      <span class="wzp-row"><Table2 size={9} strokeWidth={1.8} />{m.wizard_preview_customers()}</span>
      <span class="wzp-row"><Database size={9} strokeWidth={1.8} />{m.wizard_preview_production()}<b class="wzp-dot"></b></span>
    </span>
    <span class="wzp-sheet" class:prod={step === 'safety'}>
      <span class="wzp-tabs"><span class="on">Query 1</span><span>{m.wizard_preview_orders()}</span></span>
      <span class="wzp-tools"><span class="wzp-run"><Play size={8} strokeWidth={2} />{m.run_statement()}</span></span>
      <span class="wzp-code"><b>select</b> {m.wizard_preview_name()}, <em>count</em>(*), <em>sum</em>({m.wizard_preview_total()})<br /><b>from</b> {m.wizard_preview_orders()} <b>where</b> {m.wizard_preview_total()} &gt; <u>100</u>;</span>
      <span class="wzp-grid">
        <span class="h">{m.wizard_preview_name()}</span><span class="h n">{m.wizard_preview_orders()}</span><span class="h n">{m.wizard_preview_total()}</span>
        {#each rows as r, i (i)}
          <span class:sel={i === 1}><i style:width="{r[0]}%"></i></span><span class="n" class:sel={i === 1}><i style:width="{r[1]}%"></i></span><span class="n" class:sel={i === 1}><i style:width="{r[2]}%"></i></span>
        {/each}
      </span>
      {#if step === 'safety'}
        <span class="wzp-rules"><b class="wzp-prod">{m.env_production()}</b><span>{unlock}</span><span>{confirm}</span></span>
      {/if}
    </span>
    <span class="wzp-status"><i></i>{m.wizard_preview_rows()}</span>
  </span>
</figure>
