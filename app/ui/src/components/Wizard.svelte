<script lang="ts">
  import { Check, Plus } from '@lucide/svelte';
  import { onMount, tick } from 'svelte';
  import { appearance, LOCALES, SCALES, THEMES, updateAppearance } from '../lib/appearance.svelte';
  import { localeName, paletteName, styleName, themeLabel } from '../lib/appearance-names';
  import type { Appearance } from '../lib/api';
  import { refreshAll } from '../lib/guard.svelte';
  import { FONT_SIZES } from '../lib/prefs';
  import { prefs, updateSection } from '../lib/prefs.svelte';
  import { CONFIRM_MODES, UNLOCK_MODES } from '../lib/prodsafety';
  import { clampStep, keyIntent, LAST, STEPS, type Step } from '../lib/wizard';
  import { closeWizard } from '../lib/wizard.svelte';
  import { m } from '../paraglide/messages';
  import Segmented from './Segmented.svelte';
  import SoundVolume from './SoundVolume.svelte';
  import StylePicker from './StylePicker.svelte';
  import WizardPreview from './WizardPreview.svelte';
  import Wordmark from './Wordmark.svelte';
  import WindowControls from './WindowControls.svelte';

  let { onnewconnection }: { onnewconnection: () => void } = $props();

  let index = $state(0);
  let reached = $state(0);
  let asking = $state(false);
  let error = $state('');
  let announce = $state('');
  let root = $state<HTMLDivElement>();
  let body = $state<HTMLDivElement>();
  let heading = $state<HTMLHeadingElement>();
  let skipDialog = $state<HTMLDialogElement>();
  let keepGoing = $state<HTMLButtonElement>();

  const step = $derived<Step>(STEPS[index] ?? 'ready');
  const a = $derived(appearance.value);
  const safety = $derived(prefs.value.safety);
  const firstRun = !appearance.value.setup_done;

  const STEP_NAME: Record<Step, () => string> = {
    language: m.wizard_step_language,
    look: m.wizard_step_look,
    comfort: m.wizard_step_comfort,
    safety: m.wizard_step_safety,
    ready: m.wizard_step_ready,
  };
  const COPY: Record<Step, readonly [() => string, () => string]> = {
    language: [m.wizard_language_title, m.wizard_language_hint],
    look: [m.wizard_look_title, m.wizard_look_hint],
    comfort: [m.wizard_comfort_title, m.wizard_comfort_hint],
    safety: [m.wizard_safety_title, m.wizard_safety_hint],
    ready: [m.wizard_ready_title, m.wizard_ready_hint],
  };
  const UNLOCK_TEXT = {
    timed: [m.settings_unlock_timed, m.settings_unlock_timed_hint],
    session: [m.settings_unlock_session, m.settings_unlock_session_hint],
    open: [m.settings_unlock_open, m.settings_unlock_open_hint],
  } as const;
  const CONFIRM_TEXT = {
    typed: [m.settings_confirm_typed, m.settings_confirm_typed_hint],
    modal: [m.settings_confirm_modal, m.settings_confirm_modal_hint],
    none: [m.settings_confirm_none, m.settings_confirm_none_hint],
  } as const;

  const unlockText = $derived(safety.unlock_mode === 'timed' ? `${UNLOCK_TEXT.timed[0]()}, ${m.settings_minutes({ count: safety.unlock_minutes })}` : UNLOCK_TEXT[safety.unlock_mode][0]());
  const confirmText = $derived(CONFIRM_TEXT[safety.confirm_mode][0]());
  const summary = $derived([
    [m.settings_language(), localeName(a.locale)],
    [m.settings_style(), styleName[a.style]()],
    [m.settings_palette(), paletteName(a.style, a.palette)],
    [m.settings_theme(), themeLabel[a.theme]()],
    [m.settings_scale(), `${a.ui_scale}%`],
    [m.settings_sounds(), a.sounds ? m.wizard_sounds_at({ volume: a.sound_volume }) : m.settings_sounds_off()],
    [m.wizard_editor_font(), String(prefs.value.editor.font_size)],
    [m.settings_unlock_mode(), unlockText],
    [m.settings_confirm_mode(), confirmText],
  ] as const);

  const fail = (e: unknown) => (error = m.save_failed({ detail: e instanceof Error ? e.message : String(e) }));

  function look(patch: Partial<Appearance>) {
    error = '';
    updateAppearance(patch, { replaySplash: false }).catch(fail);
  }

  function editor(font_size: number) {
    error = '';
    updateSection('editor', { font_size }).catch(fail);
  }

  function guard(patch: Partial<typeof safety>) {
    error = '';
    updateSection('safety', patch, true).then(refreshAll).catch(fail);
  }

  async function go(i: number) {
    index = clampStep(i);
    reached = Math.max(reached, index);
    announce = m.wizard_announce({ n: index + 1, total: STEPS.length, name: STEP_NAME[step]() });
    await tick();
    focusStep();
  }

  function focusStep() {
    const choice = body?.querySelector<HTMLElement>('[aria-checked="true"], input:checked');
    (choice ?? heading)?.focus();
  }

  async function finish(then?: () => void) {
    asking = false;
    try {
      await closeWizard();
      then?.();
    } catch (e) {
      fail(e);
    }
  }

  function askSkip() {
    if (!firstRun) return void finish();
    asking = true;
  }

  $effect(() => {
    if (asking && skipDialog && !skipDialog.open) {
      skipDialog.showModal();
      void tick().then(() => keepGoing?.focus());
    }
    if (!asking && skipDialog?.open) skipDialog.close();
  });

  const splashUp = () => !!document.querySelector('.splash:not([data-leaving])');

  function trap(e: KeyboardEvent) {
    const all = [...(root?.querySelectorAll<HTMLElement>('button:not([disabled]):not([tabindex="-1"]), input:not([disabled]):not([tabindex="-1"])') ?? [])].filter((el) => !el.closest('dialog'));
    const first = all[0];
    const last = all.at(-1);
    if (!first || !last) return;
    if (e.shiftKey && document.activeElement === first) {
      e.preventDefault();
      last.focus();
    } else if (!e.shiftKey && document.activeElement === last) {
      e.preventDefault();
      first.focus();
    }
  }

  function onkeydown(e: KeyboardEvent) {
    if (asking || (e.target as Element | null)?.closest('dialog')) return;
    e.stopPropagation();
    if (splashUp()) return;
    if (e.key === 'Tab') return trap(e);
    const intent = keyIntent(e, e.target as Element | null);
    if (!intent) return;
    e.preventDefault();
    if (intent === 'skip') askSkip();
    else if (index < LAST) void go(index + 1);
    else void finish();
  }

  onMount(() => {
    const before = document.activeElement as HTMLElement | null;
    const html = document.documentElement;
    const shown = new MutationObserver(() => html.hasAttribute('data-ready') && (shown.disconnect(), focusStep()));
    if (html.hasAttribute('data-ready')) focusStep();
    else shown.observe(html, { attributes: true, attributeFilter: ['data-ready'] });
    return () => {
      shown.disconnect();
      requestAnimationFrame(() => before?.isConnected && before.focus());
    };
  });
</script>

<div class="shell wizard" role="dialog" aria-modal="true" aria-labelledby="wz-title" aria-describedby="wz-hint" tabindex="-1" bind:this={root} {onkeydown}>
  <header class="titlebar" data-tauri-drag-region="deep">
    <Wordmark />
    <span class="titlebar-sep" aria-hidden="true"></span>
    <h1 class="titlebar-title">{m.wizard_title()}</h1>
    <span class="spacer"></span>
    <WindowControls />
  </header>

  <nav class="sidebar wz-rail" aria-label={m.wizard_steps()}>
    <div class="sidebar-head"><span class="caps">{m.wizard_steps()}</span></div>
    <ol class="wz-steps">
      {#each STEPS as s, i (s)}
        <li class="conn" class:on={i === index} class:wz-done={i < index}>
          <button class="conn-main" type="button" aria-current={i === index ? 'step' : undefined} disabled={i > reached} onclick={() => go(i)}>
            <span class="conn-mark wz-num" aria-hidden="true">{#if i < index}<Check size={13} strokeWidth={2} />{:else}{i + 1}{/if}</span>
            <span class="conn-name">{STEP_NAME[s]()}</span>
          </button>
        </li>
      {/each}
    </ol>
  </nav>

  <main class="sheet wz-sheet">
    <section class="screen wz-screen" class:wz-wide={step === 'look'} data-step={step}>
      <div class="wz-main">
        <header class="wz-head">
          <p class="caps">{m.wizard_step_of({ n: index + 1, total: STEPS.length })}</p>
          <h2 class="display" id="wz-title" tabindex="-1" bind:this={heading}>{COPY[step][0]()}</h2>
          <p class="hint" id="wz-hint">{COPY[step][1]()}</p>
        </header>

        <div class="wz-body" bind:this={body}>
          {#if step === 'language'}
            <fieldset class="choices wz-choices">
              <legend class="caps">{m.settings_language()}</legend>
              {#each LOCALES as l (l)}
                <label class="choice" class:on={a.locale === l} lang={l}>
                  <input type="radio" name="wz-locale" value={l} checked={a.locale === l} onchange={() => look({ locale: l })} />
                  <span class="choice-name">{localeName(l)}</span>
                </label>
              {/each}
            </fieldset>
          {:else if step === 'look'}
            <StylePicker id="wz" hint={false} onpick={look} />
            <div class="setting row">
              <h3 class="caps">{m.settings_theme()}</h3>
              <Segmented label={m.settings_theme()} value={a.theme} options={THEMES.map((t) => ({ value: t, label: themeLabel[t]() }))} onchange={(theme) => look({ theme })} />
            </div>
          {:else if step === 'comfort'}
            <div class="setting row">
              <h3 class="caps">{m.settings_scale()}</h3>
              <Segmented label={m.settings_scale()} value={String(a.ui_scale)} options={SCALES.map((v) => ({ value: String(v), label: `${v}%` }))} onchange={(v) => look({ ui_scale: Number(v) })} />
            </div>
            <div class="setting row">
              <h3 class="caps">{m.settings_sounds()}</h3>
              <Segmented
                label={m.settings_sounds()}
                value={a.sounds ? 'on' : 'off'}
                options={[
                  { value: 'on', label: m.settings_sounds_on() },
                  { value: 'off', label: m.settings_sounds_off() },
                ]}
                onchange={(v) => look({ sounds: v === 'on' })}
              />
            </div>
            <div class="setting row">
              <h3 class="caps"><label for="wz-volume">{m.settings_volume()}</label></h3>
              <SoundVolume id="wz-volume" onvolume={(sound_volume) => look({ sound_volume })} />
            </div>
            <div class="setting row">
              <h3 class="caps">{m.wizard_editor_font()}</h3>
              <Segmented label={m.wizard_editor_font()} value={String(prefs.value.editor.font_size)} options={FONT_SIZES.map((v) => ({ value: String(v), label: String(v) }))} onchange={(v) => editor(Number(v))} />
            </div>
          {:else if step === 'safety'}
            <fieldset class="choices wz-choices" aria-describedby="wz-unlock-hint">
              <legend class="caps">{m.settings_unlock_mode()}</legend>
              {#each UNLOCK_MODES as mode (mode)}
                <label class="choice" class:risky={mode === 'open'} class:on={safety.unlock_mode === mode}>
                  <input type="radio" name="wz-unlock" value={mode} checked={safety.unlock_mode === mode} onchange={() => guard({ unlock_mode: mode })} />
                  <span class="choice-name">{UNLOCK_TEXT[mode][0]()}</span>
                </label>
              {/each}
              <p class="hint" id="wz-unlock-hint">{safety.unlock_mode === 'timed' ? m.wizard_unlock_timed_hint({ count: safety.unlock_minutes }) : UNLOCK_TEXT[safety.unlock_mode][1]()}</p>
            </fieldset>
            <fieldset class="choices wz-choices" aria-describedby="wz-confirm-hint">
              <legend class="caps">{m.settings_confirm_mode()}</legend>
              {#each CONFIRM_MODES as mode (mode)}
                <label class="choice" class:risky={mode === 'none'} class:on={safety.confirm_mode === mode}>
                  <input type="radio" name="wz-confirm" value={mode} checked={safety.confirm_mode === mode} onchange={() => guard({ confirm_mode: mode })} />
                  <span class="choice-name">{CONFIRM_TEXT[mode][0]()}</span>
                </label>
              {/each}
              <p class="hint" id="wz-confirm-hint">{CONFIRM_TEXT[safety.confirm_mode][1]()}</p>
            </fieldset>
          {:else}
            <dl class="wz-summary">
              {#each summary as [label, value] (label)}
                <div class="wz-sum-row">
                  <dt class="caps">{label}</dt>
                  <dd>{value}</dd>
                </div>
              {/each}
            </dl>
          {/if}
          {#if error}<p class="error" role="alert">{error}</p>{/if}
        </div>
      </div>

      <WizardPreview {step} unlock={unlockText} confirm={confirmText} />
    </section>

    <footer class="wz-foot">
      {#if step !== 'ready'}
        <button class="btn ghost" type="button" onclick={() => finish()}>{m.wizard_skip()}<kbd>Esc</kbd></button>
      {/if}
      <span class="spacer"></span>
      {#if index > 0}<button class="btn" type="button" onclick={() => go(index - 1)}>{m.wizard_back()}</button>{/if}
      {#if step === 'ready'}
        <button class="btn" type="button" onclick={() => finish(onnewconnection)}><Plus size={14} strokeWidth={2} aria-hidden="true" />{m.conn_new()}</button>
        <button class="btn primary" type="button" onclick={() => finish()}>{m.wizard_start()}<kbd>↵</kbd></button>
      {:else}
        <button class="btn primary" type="button" onclick={() => go(index + 1)}>{m.wizard_next()}<kbd>↵</kbd></button>
      {/if}
    </footer>
  </main>

  <footer class="statusbar">
    <span>{m.wizard_step_of({ n: index + 1, total: STEPS.length })} · {STEP_NAME[step]()}</span>
    <span class="spacer"></span>
    <span>{m.wizard_keys()}</span>
    <span>NiceDB 0.1</span>
  </footer>

  <p class="wz-live" aria-live="polite">{announce}</p>

  <dialog class="sheet-dialog wz-skip" bind:this={skipDialog} oncancel={(e) => (e.preventDefault(), (asking = false))} aria-labelledby="wz-skip-title" aria-describedby="wz-skip-body">
    <header class="dialog-head">
      <h2 class="display" id="wz-skip-title">{m.wizard_skip_title()}</h2>
    </header>
    <div class="dialog-body">
      <p id="wz-skip-body">{m.wizard_skip_body()}</p>
    </div>
    <footer class="dialog-foot">
      <span class="spacer"></span>
      <button class="btn" type="button" bind:this={keepGoing} onclick={() => (asking = false)}>{m.wizard_skip_cancel()}</button>
      <button class="btn primary" type="button" onclick={() => finish()}>{m.wizard_skip_confirm()}</button>
    </footer>
  </dialog>
</div>
