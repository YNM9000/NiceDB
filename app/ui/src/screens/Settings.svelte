<script lang="ts" module>
  export type SettingsSection = 'look' | 'voice' | 'editor' | 'results' | 'safety' | 'keys' | 'drivers' | 'updates' | 'about';
  const SECTIONS: SettingsSection[] = ['look', 'voice', 'editor', 'results', 'safety', 'keys', 'drivers', 'updates', 'about'];
  const asked = typeof location === 'undefined' ? null : new URLSearchParams(location.search).get('section');
  const nav = $state({ section: (SECTIONS.find((s) => s === asked) ?? 'look') as SettingsSection });
  export function showSettingsSection(section: SettingsSection) {
    nav.section = section;
  }
</script>

<script lang="ts">
  import { CircleArrowUp, Info, Keyboard, Languages, Palette, RotateCcw, ShieldCheck, SquareCode, Table2 } from '@lucide/svelte';
  import { tick } from 'svelte';
  import Segmented from '../components/Segmented.svelte';
  import Maker from '../components/Maker.svelte';
  import SettingsAbout from '../components/SettingsAbout.svelte';
  import SettingsPrefs from '../components/SettingsPrefs.svelte';
  import { ICON as ACTION_ICON } from '../lib/icons';
  import SettingsUpdates from '../components/SettingsUpdates.svelte';
  import SoundBoard from '../components/SoundBoard.svelte';
  import SoundVolume from '../components/SoundVolume.svelte';
  import StylePicker from '../components/StylePicker.svelte';
  import { appearance, LOCALES, SCALES, THEMES, updateAppearance } from '../lib/appearance.svelte';
  import { localeName, themeLabel } from '../lib/appearance-names';
  import type { Appearance } from '../lib/api';
  import { preview } from '../lib/sound';
  import { openWizard } from '../lib/wizard.svelte';
  import { m } from '../paraglide/messages';

  let error = $state('');
  let page = $state<HTMLDivElement>();
  const a = $derived(appearance.value);

  const NAME: Record<SettingsSection, () => string> = {
    look: m.settings_appearance,
    voice: m.settings_voice,
    editor: m.settings_editor,
    results: m.settings_results,
    safety: m.settings_safety,
    keys: m.settings_keys,
    drivers: m.settings_drivers,
    updates: m.settings_updates,
    about: m.settings_about,
  };
  const ICON = { look: Palette, voice: Languages, editor: SquareCode, results: Table2, safety: ShieldCheck, keys: Keyboard, drivers: ACTION_ICON.connect, updates: CircleArrowUp, about: Info };

  function set(patch: Partial<Appearance>) {
    error = '';
    updateAppearance(patch).catch((e: unknown) => {
      error = m.save_failed({ detail: e instanceof Error ? e.message : String(e) });
    });
  }

  function open(section: SettingsSection, focus = false) {
    nav.section = section;
    page?.closest('.sheet')?.scrollTo({ top: 0 });
    if (focus) void tick().then(() => document.getElementById(`set-tab-${section}`)?.focus());
  }

  function onkeydown(e: KeyboardEvent) {
    const i = SECTIONS.indexOf(nav.section);
    const to = { ArrowDown: i + 1, ArrowUp: i - 1, Home: 0, End: SECTIONS.length - 1 }[e.key];
    if (to === undefined) return;
    e.preventDefault();
    open(SECTIONS[(to + SECTIONS.length) % SECTIONS.length] ?? 'look', true);
  }
</script>

<section class="screen settings">
  <nav class="settings-nav" aria-label={m.settings_sections()}>
    <div class="settings-tabs" role="tablist" aria-orientation="vertical" aria-label={m.settings_sections()} tabindex="-1" {onkeydown}>
      {#each SECTIONS as s, i (s)}
        {@const Icon = ICON[s]}
        <button
          type="button"
          role="tab"
          id="set-tab-{s}"
          class="settings-tab"
          aria-selected={nav.section === s}
          aria-controls="set-page"
          tabindex={nav.section === s ? 0 : -1}
          onclick={() => open(s)}
        >
          <span class="settings-tab-n" aria-hidden="true">{String(i + 1).padStart(2, '0')}</span>
          <Icon size={14} strokeWidth={1.7} aria-hidden="true" />
          <span class="settings-tab-name">{NAME[s]()}</span>
        </button>
      {/each}
    </div>
  </nav>

  <div class="settings-page" id="set-page" role="tabpanel" aria-labelledby="set-tab-{nav.section}" bind:this={page}>
    {#if nav.section === 'look'}
      <header class="settings-head">
        <h2 class="display">{m.settings_appearance()}</h2>
        <button class="btn ghost compact" type="button" onclick={openWizard}><RotateCcw size={12} strokeWidth={1.8} aria-hidden="true" />{m.wizard_reopen()}</button>
      </header>

      <StylePicker id="lbl" onpick={set} />

      <div class="setting-rows">
        <div class="setting row">
          <h3 class="caps">{m.settings_theme()}</h3>
          <Segmented label={m.settings_theme()} value={a.theme} options={THEMES.map((t) => ({ value: t, label: themeLabel[t]() }))} onchange={(theme) => set({ theme })} />
        </div>
        <div class="setting row">
          <h3 class="caps">{m.settings_scale()}</h3>
          <Segmented label={m.settings_scale()} value={String(a.ui_scale)} options={SCALES.map((v) => ({ value: String(v), label: `${v}%` }))} onchange={(v) => set({ ui_scale: Number(v) })} />
        </div>
      </div>
    {:else if nav.section === 'voice'}
      <header class="settings-head">
        <h2 class="display">{m.settings_voice()}</h2>
      </header>
      <div class="setting-rows">
        <div class="setting row">
          <h3 class="caps">{m.settings_language()}</h3>
          <Segmented label={m.settings_language()} value={a.locale} options={LOCALES.map((l) => ({ value: l, label: localeName(l) }))} onchange={(locale) => set({ locale })} />
        </div>
        <div class="setting row">
          <h3 class="caps">{m.settings_sounds()}</h3>
          <Segmented
            label={m.settings_sounds()}
            value={a.sounds ? a.sound_detail : 'off'}
            options={[
              { value: 'all', label: m.settings_sounds_all() },
              { value: 'reduced', label: m.settings_sounds_reduced() },
              { value: 'off', label: m.settings_sounds_off() },
            ] as const}
            onchange={(v) => set(v === 'off' ? { sounds: false } : { sounds: true, sound_detail: v })}
          />
        </div>
        <p class="hint sound-hint">{m.settings_sounds_hint()}</p>
        <div class="setting row">
          <h3 class="caps">{m.settings_timbre()}</h3>
          <Segmented
            label={m.settings_timbre()}
            value={a.sound_timbre}
            options={[
              { value: 'marimba', label: m.settings_timbre_marimba() },
              { value: 'ciotola', label: m.settings_timbre_ciotola() },
            ] as const}
            onchange={(sound_timbre) => {
              set({ sound_timbre });
              preview('done');
            }}
          />
        </div>
        <div class="setting row">
          <h3 class="caps"><label for="sound-volume">{m.settings_volume()}</label></h3>
          <SoundVolume id="sound-volume" onvolume={(sound_volume) => set({ sound_volume })} />
        </div>
        <div class="setting row">
          <h3 class="caps">{m.settings_sound_gestures_switch()}</h3>
          <Segmented
            label={m.settings_sound_gestures_switch()}
            value={a.sound_gestures ? 'on' : 'off'}
            options={[
              { value: 'on', label: m.settings_sounds_on() },
              { value: 'off', label: m.settings_sounds_off() },
            ]}
            onchange={(v) => set({ sound_gestures: v === 'on' })}
          />
        </div>
        <div class="setting row">
          <h3 class="caps">{m.settings_jingle()}</h3>
          <Segmented
            label={m.settings_jingle()}
            value={a.jingle ? 'on' : 'off'}
            options={[
              { value: 'on', label: m.settings_jingle_on() },
              { value: 'off', label: m.settings_jingle_off() },
            ]}
            onchange={(v) => set({ jingle: v === 'on' })}
          />
        </div>
        <SoundBoard />
      </div>
    {:else if nav.section === 'updates'}
      <SettingsUpdates />
    {:else if nav.section === 'about'}
      <SettingsAbout />
    {:else}
      <SettingsPrefs section={nav.section} />
    {/if}

    {#if error}<p class="error" role="alert">{error}</p>{/if}
  </div>
  <Maker />
</section>
