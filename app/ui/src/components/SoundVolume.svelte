<script lang="ts">
  import { appearance } from '../lib/appearance.svelte';
  import { preview } from '../lib/sound';
  import { m } from '../paraglide/messages';

  let { id, onvolume }: { id: string; onvolume: (percent: number) => void } = $props();
  let level = $state<number | null>(null);
  const a = $derived(appearance.value);
</script>

<div class="volume">
  <input
    {id}
    type="range"
    min="0"
    max="100"
    step="5"
    value={a.sound_volume}
    disabled={!a.sounds}
    oninput={(e) => (level = e.currentTarget.valueAsNumber)}
    onchange={(e) => {
      level = null;
      onvolume(e.currentTarget.valueAsNumber);
    }}
  />
  <span class="volume-num" aria-hidden="true">{level ?? a.sound_volume}%</span>
  <button class="btn" type="button" disabled={!a.sounds} onclick={() => preview('done')}>{m.settings_sound_try()}</button>
</div>
