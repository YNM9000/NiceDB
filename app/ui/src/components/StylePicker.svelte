<script lang="ts">
  import { appearance, PALETTES, resolvedTheme, STYLES } from '../lib/appearance.svelte';
  import { arrowPick, paletteName, styleDesc, styleName } from '../lib/appearance-names';
  import type { Appearance } from '../lib/api';
  import { m } from '../paraglide/messages';

  let { id, hint = true, onpick }: { id: string; hint?: boolean; onpick: (patch: Partial<Appearance>) => void } = $props();

  const a = $derived(appearance.value);
  const theme = $derived(resolvedTheme(a, appearance.systemDark));
</script>

<div class="setting">
  <h3 class="caps" id="{id}-style">{m.settings_style()}</h3>
  {#if hint}<p class="hint">{m.settings_style_hint()}</p>{/if}
  <div
    class="style-cards"
    role="radiogroup"
    aria-labelledby="{id}-style"
    tabindex="-1"
    onkeydown={(e) => arrowPick(e, STYLES, a.style, (style) => onpick({ style }))}
  >
    {#each STYLES as style (style)}
      <button
        type="button"
        role="radio"
        class="style-card"
        aria-checked={a.style === style}
        tabindex={a.style === style ? 0 : -1}
        onclick={() => onpick({ style })}
      >
        <span class="preview" data-style={style} data-palette={a.style === style ? a.palette : '1'} data-theme={theme}>
          <span class="pv-chrome"></span>
          <span class="pv-sheet">
            <span class="pv-title">{styleName[style]()}</span>
            <span class="pv-line"></span>
            <span class="pv-line short"></span>
            <span class="pv-accent"></span>
          </span>
        </span>
        <span class="style-name">{styleName[style]()}</span>
        <span class="style-desc">{styleDesc[style]()}</span>
      </button>
    {/each}
  </div>
</div>

<div class="setting">
  <h3 class="caps" id="{id}-palette">{m.settings_palette()}</h3>
  <div
    class="palettes"
    role="radiogroup"
    aria-labelledby="{id}-palette"
    tabindex="-1"
    onkeydown={(e) => arrowPick(e, PALETTES, a.palette, (palette) => onpick({ palette }))}
  >
    {#each PALETTES as palette (palette)}
      <button
        type="button"
        role="radio"
        class="palette"
        aria-checked={a.palette === palette}
        tabindex={a.palette === palette ? 0 : -1}
        onclick={() => onpick({ palette })}
      >
        <span class="chips" data-style={a.style} data-palette={palette} data-theme={theme} aria-hidden="true">
          <i class="chip-chrome"></i><i class="chip-sheet"></i><i class="chip-ink"></i><i class="chip-accent"></i>
        </span>
        <span>{paletteName(a.style, palette)}</span>
      </button>
    {/each}
  </div>
</div>
