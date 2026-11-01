<script lang="ts">
  import { appearance } from '../lib/appearance.svelte';
  import { ESSENTIAL, GESTURES, OUTCOMES, preview, type Clip, type SoundName } from '../lib/sound';
  import { m } from '../paraglide/messages';

  const NAME: Record<Clip, () => string> = {
    done: m.sound_done,
    applied: m.sound_applied,
    error: m.sound_error,
    warn: m.sound_warn,
    notify: m.sound_notify,
    unlock: m.sound_unlock,
    lock: m.sound_lock,
    copy: m.sound_copy,
    toggle: m.sound_toggle,
    drop: m.sound_drop,
    jingle: m.sound_jingle,
  };

  const GROUPS: readonly (readonly Clip[])[] = [OUTCOMES, [...GESTURES, 'jingle']];

  const a = $derived(appearance.value);
  const reduced = $derived(a.sound_detail === 'reduced');

  function quiet(clip: Clip): string | undefined {
    if (clip === 'jingle') return !a.jingle ? m.settings_jingle_quiet() : reduced ? m.settings_sound_quiet() : undefined;
    if ((GESTURES as readonly Clip[]).includes(clip) && !a.sound_gestures) return m.settings_sound_gesture_quiet();
    return reduced && !ESSENTIAL.includes(clip as SoundName) ? m.settings_sound_quiet() : undefined;
  }
</script>

<div class="setting row sound-row">
  <h3 class="caps">{m.settings_sound_board()}</h3>
  <div class="sound-board" role="group" aria-label={m.settings_sound_board()}>
    {#each GROUPS as clips, i (i)}
      <div class="seg sound-keys">
        {#each clips as clip (clip)}
          {@const why = quiet(clip)}
          <button class="sound-key" class:quiet={why !== undefined} type="button" data-sound={clip} disabled={!a.sounds} data-tip={why} onclick={() => preview(clip)}>{NAME[clip]()}</button>
        {/each}
      </div>
    {/each}
  </div>
</div>
