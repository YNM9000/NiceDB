<script lang="ts">
  import { Lock, RotateCcw } from '@lucide/svelte';
  import { keys } from '../lib/contextmenu.svelte';
  import { refreshAll } from '../lib/guard.svelte';
  import { CONFIRM_MODES, UNLOCK_MODES } from '../lib/prodsafety';
  import { FONT_SIZES, ROW_CAPS, TAB_WIDTHS, TIMEOUTS, UNLOCKS } from '../lib/prefs';
  import { currentBindings, prefs, resetPrefs, setKeys, updateSection, type Section } from '../lib/prefs.svelte';
  import { ACTIONS, FIXED, comboOf, findConflict, overridesAfter, shown as order, usable, type ActionId } from '../lib/shortcuts';
  import { m } from '../paraglide/messages';
  import Segmented from './Segmented.svelte';
  import { api } from '../lib/api';
  import { ICON } from '../lib/icons';
  import { savePrefs } from '../lib/prefs.svelte';

  let { section }: { section: 'editor' | 'results' | 'safety' | 'keys' | 'drivers' } = $props();
  import Select from './Select.svelte';

  const editor = $derived(prefs.value.editor);
  const results = $derived(prefs.value.results);
  const safety = $derived(prefs.value.safety);
  const drivers = $derived(prefs.value.drivers);
  const setClidriver = (path: string) => run(savePrefs({ ...prefs.value, drivers: { ...prefs.value.drivers, db2_clidriver: path.trim() } }));
  async function browse() {
    const path = await api.pickFolder('clidriver');
    if (path) setClidriver(path);
  }
  const bound = $derived(currentBindings());

  let error = $state('');
  let capturing = $state<ActionId | null>(null);
  let refusal = $state<{ id: ActionId; text: string } | null>(null);

  const fail = (e: unknown) => (error = m.save_failed({ detail: e instanceof Error ? e.message : String(e) }));
  const run = (work: Promise<unknown>) => {
    error = '';
    work.catch(fail);
  };
  const patch = <S extends Section>(section: S, p: Partial<(typeof prefs.value)[S]>) => run(updateSection(section, p).then(() => (section === 'safety' ? refreshAll() : undefined)));

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

  const ACTION_NAME: Record<ActionId, () => string> = {
    run_statement: m.key_run_statement,
    run_script: m.key_run_script,
    dry_run: m.key_dry_run,
    cancel: m.key_cancel,
    format: m.key_format,
    rename_alias: m.key_rename_alias,
    save_script: m.key_save_script,
    panel_notes: m.key_panel_notes,
    panel_history: m.key_panel_history,
    new_tab: m.key_new_tab,
    palette: m.key_palette,
    palette_commands: m.key_palette_commands,
  };
  const FIXED_NAME: Record<string, () => string> = {
    undo: m.key_undo,
    redo: m.key_redo,
    redo_alt: m.key_redo,
    space: m.key_space,
    copy: m.menu_copy,
    paste: m.menu_paste,
    cut: m.menu_cut,
    select_all: m.menu_select_all,
    find: m.key_find,
  };

  const timeoutLabel = (s: number) => (s === 0 ? m.settings_no_limit() : s < 60 ? `${s} s` : s < 3600 ? `${s / 60} min` : `${s / 3600} h`);
  const capLabel = (n: number) => (n >= 1_000_000 ? `${n / 1_000_000} M` : n.toLocaleString('it-IT'));

  const ENVS = [
    ['timeout_local', m.env_local],
    ['timeout_development', m.env_development],
    ['timeout_staging', m.env_staging],
    ['timeout_production', m.env_production],
  ] as const;

  function show(combo: string) {
    return combo ? keys(order(combo)).replace('⇧', m.key_shift()).replace('↵', m.key_enter()) : m.key_unbound();
  }

  function listen(node: HTMLButtonElement, id: ActionId) {
    const onkey = (e: KeyboardEvent) => {
      e.preventDefault();
      e.stopPropagation();
      if (e.key === 'Escape') {
        capturing = null;
        return;
      }
      const combo = comboOf(e);
      if (!combo) return;
      if (!usable(combo)) {
        refusal = { id, text: m.key_needs_modifier() };
        return;
      }
      const clash = findConflict(bound, id, combo);
      if (clash) {
        const who = clash.kind === 'action' ? ACTION_NAME[clash.id]() : (FIXED_NAME[clash.key] ?? (() => clash.key))();
        refusal = { id, text: m.key_conflict({ keys: show(combo), name: who }) };
        return;
      }
      refusal = null;
      capturing = null;
      run(setKeys(overridesAfter(prefs.value.keys, id, combo)));
    };
    node.addEventListener('keydown', onkey);
    node.focus();
    return { destroy: () => node.removeEventListener('keydown', onkey) };
  }

  function begin(id: ActionId) {
    refusal = null;
    capturing = capturing === id ? null : id;
  }

  const reset = (id: ActionId) => run(setKeys(overridesAfter(prefs.value.keys, id, null)));
  const unbind = (id: ActionId) => run(setKeys(overridesAfter(prefs.value.keys, id, '')));
  const changed = (id: ActionId) => id in prefs.value.keys;
</script>

<div class="prefs">
  {#if section === 'editor'}
  <section class="pref-section" aria-labelledby="pref-editor">
    <header class="pref-head">
      <h2 class="display" id="pref-editor">{m.settings_editor()}</h2>
      <button class="btn ghost compact" type="button" onclick={() => run(resetPrefs('editor'))}><RotateCcw size={12} strokeWidth={1.8} aria-hidden="true" />{m.settings_reset()}</button>
    </header>
    <div class="setting row">
      <h3 class="caps">{m.settings_font_size()}</h3>
      <Segmented label={m.settings_font_size()} value={String(editor.font_size)} options={FONT_SIZES.map((v) => ({ value: String(v), label: String(v) }))} onchange={(v) => patch('editor', { font_size: Number(v) })} />
    </div>
    <div class="setting row">
      <h3 class="caps">{m.settings_tab_width()}</h3>
      <Segmented label={m.settings_tab_width()} value={String(editor.tab_width)} options={TAB_WIDTHS.map((v) => ({ value: String(v), label: String(v) }))} onchange={(v) => patch('editor', { tab_width: Number(v) })} />
    </div>
    <div class="setting row">
      <h3 class="caps">{m.settings_autocomplete()}</h3>
      <Segmented
        label={m.settings_autocomplete()}
        value={editor.autocomplete ? 'on' : 'off'}
        options={[
          { value: 'on', label: m.settings_sounds_on() },
          { value: 'off', label: m.settings_sounds_off() },
        ]}
        onchange={(v) => patch('editor', { autocomplete: v === 'on' })}
      />
    </div>
    <div class="setting row">
      <h3 class="caps">{m.settings_uppercase()}</h3>
      <Segmented
        label={m.settings_uppercase()}
        value={editor.uppercase_keywords ? 'on' : 'off'}
        options={[
          { value: 'on', label: m.settings_sounds_on() },
          { value: 'off', label: m.settings_sounds_off() },
        ]}
        onchange={(v) => patch('editor', { uppercase_keywords: v === 'on' })}
      />
    </div>
    <div class="setting row">
      <h3 class="caps">{m.settings_inspections()}</h3>
      <Segmented
        label={m.settings_inspections()}
        value={editor.inspections ? 'on' : 'off'}
        options={[
          { value: 'on', label: m.settings_sounds_on() },
          { value: 'off', label: m.settings_sounds_off() },
        ]}
        onchange={(v) => patch('editor', { inspections: v === 'on' })}
      />
    </div>
    <div class="setting row">
      <h3 class="caps">{m.settings_on_error()}</h3>
      <Segmented
        label={m.settings_on_error()}
        value={editor.continue_on_error ? 'go' : 'stop'}
        options={[
          { value: 'stop', label: m.settings_on_error_stop() },
          { value: 'go', label: m.settings_on_error_go() },
        ]}
        onchange={(v) => patch('editor', { continue_on_error: v === 'go' })}
      />
    </div>
  </section>
  {:else if section === 'results'}
  <section class="pref-section" aria-labelledby="pref-results">
    <header class="pref-head">
      <h2 class="display" id="pref-results">{m.settings_results()}</h2>
      <button class="btn ghost compact" type="button" onclick={() => run(resetPrefs('results'))}><RotateCcw size={12} strokeWidth={1.8} aria-hidden="true" />{m.settings_reset()}</button>
    </header>
    <div class="setting row">
      <h3 class="caps">{m.settings_row_cap()}</h3>
      <Segmented label={m.settings_row_cap()} value={String(results.row_cap)} options={ROW_CAPS.map((v) => ({ value: String(v), label: capLabel(v) }))} onchange={(v) => patch('results', { row_cap: Number(v) })} />
    </div>
    <p class="hint">{m.settings_row_cap_hint()}</p>
    <div class="setting row">
      <h3 class="caps">{m.settings_null()}</h3>
      <Segmented
        label={m.settings_null()}
        value={results.null_display}
        options={[
          { value: 'null', label: 'NULL' },
          { value: 'dash', label: '—' },
          { value: 'empty', label: m.settings_null_empty() },
        ] as const}
        onchange={(v) => patch('results', { null_display: v })}
      />
    </div>
    <div class="setting row">
      <h3 class="caps">{m.settings_dates()}</h3>
      <Segmented
        label={m.settings_dates()}
        value={results.dates}
        options={[
          { value: 'raw', label: m.settings_as_sent() },
          { value: 'local', label: m.settings_dates_local() },
        ] as const}
        onchange={(v) => patch('results', { dates: v })}
      />
    </div>
    <div class="setting row">
      <h3 class="caps">{m.settings_numbers()}</h3>
      <Segmented
        label={m.settings_numbers()}
        value={results.numbers}
        options={[
          { value: 'raw', label: m.settings_as_sent() },
          { value: 'grouped', label: m.settings_numbers_grouped() },
        ] as const}
        onchange={(v) => patch('results', { numbers: v })}
      />
    </div>
    <p class="hint">{m.settings_display_hint()}</p>
  </section>
  {:else if section === 'safety'}
  <section class="pref-section" aria-labelledby="pref-safety">
    <header class="pref-head">
      <h2 class="display" id="pref-safety">{m.settings_safety()}</h2>
      <button class="btn ghost compact" type="button" onclick={() => run(resetPrefs('safety'))}><RotateCcw size={12} strokeWidth={1.8} aria-hidden="true" />{m.settings_reset()}</button>
    </header>
    <p class="hint">{m.settings_timeout_hint()}</p>
    {#each ENVS as [field, label] (field)}
      <div class="setting row">
        <h3 class="caps"><label for="pref-{field}">{m.settings_timeout({ env: label() })}</label></h3>
        <Select id="pref-{field}" class="pref-select" value={safety[field]} onchange={(v) => patch('safety', { [field]: v })} options={[...new Set<number>([...TIMEOUTS, safety[field]])].sort((a, b) => (a === 0 ? 1 : b === 0 ? -1 : a - b)).map((t) => ({ value: t, label: timeoutLabel(t) }))} />
      </div>
    {/each}
    <fieldset class="choices" id="pref-unlock-mode">
      <legend class="caps">{m.settings_unlock_mode()}</legend>
      <p class="hint">{m.settings_unlock_mode_hint()}</p>
      {#each UNLOCK_MODES as mode (mode)}
        <label class="choice" class:risky={mode === 'open'} class:on={safety.unlock_mode === mode}>
          <input type="radio" name="unlock-mode" value={mode} checked={safety.unlock_mode === mode} onchange={() => patch('safety', { unlock_mode: mode })} />
          <span class="choice-name">{UNLOCK_TEXT[mode][0]()}</span>
          <span class="choice-hint">{UNLOCK_TEXT[mode][1]()}</span>
        </label>
      {/each}
    </fieldset>
    <div class="setting row" class:off={safety.unlock_mode !== 'timed'}>
      <h3 class="caps"><label for="pref-unlock">{m.settings_unlock()}</label></h3>
      <Select id="pref-unlock" class="pref-select" disabled={safety.unlock_mode !== 'timed'} value={safety.unlock_minutes} onchange={(v) => patch('safety', { unlock_minutes: v })} options={[...new Set<number>([...UNLOCKS, safety.unlock_minutes])].sort((a, b) => a - b).map((u) => ({ value: u, label: m.settings_minutes({ count: u }) }))} />
    </div>
    <p class="hint">{m.settings_unlock_hint()}</p>
    <fieldset class="choices" id="pref-confirm-mode">
      <legend class="caps">{m.settings_confirm_mode()}</legend>
      <p class="hint">{m.settings_confirm_mode_hint()}</p>
      {#each CONFIRM_MODES as mode (mode)}
        <label class="choice" class:risky={mode === 'none'} class:on={safety.confirm_mode === mode}>
          <input type="radio" name="confirm-mode" value={mode} checked={safety.confirm_mode === mode} onchange={() => patch('safety', { confirm_mode: mode })} />
          <span class="choice-name">{CONFIRM_TEXT[mode][0]()}</span>
          <span class="choice-hint">{CONFIRM_TEXT[mode][1]()}</span>
        </label>
      {/each}
    </fieldset>
  </section>
  {:else if section === 'drivers'}
  <section class="pref-section" aria-labelledby="pref-drivers">
    <header class="pref-head">
      <h2 class="display" id="pref-drivers">{m.settings_drivers()}</h2>
    </header>
    <p class="hint">{m.settings_drivers_hint()}</p>
    <div class="setting row pref-path">
      <h3 class="caps">{m.settings_db2_clidriver()}</h3>
      <span class="field">
        <input value={drivers.db2_clidriver} placeholder={m.settings_db2_placeholder()} autocomplete="off" spellcheck="false" onchange={(e) => setClidriver(e.currentTarget.value)} />
      </span>
      <button class="btn compact" type="button" onclick={browse}><ICON.space size={13} strokeWidth={1.8} aria-hidden="true" />{m.form_choose()}</button>
    </div>
    <p class="hint">{m.settings_db2_hint()}</p>
  </section>
  {:else}
  <section class="pref-section" aria-labelledby="pref-keys">
    <header class="pref-head">
      <h2 class="display" id="pref-keys">{m.settings_keys()}</h2>
      <button class="btn ghost compact" type="button" disabled={Object.keys(prefs.value.keys).length === 0} onclick={() => run(resetPrefs('keys'))}><RotateCcw size={12} strokeWidth={1.8} aria-hidden="true" />{m.settings_reset_all()}</button>
    </header>
    <p class="hint">{m.settings_keys_hint()}</p>
    <ul class="key-list">
      {#each ACTIONS as a (a.id)}
        <li class="key-row" class:changed={changed(a.id)}>
          <span class="key-name">{ACTION_NAME[a.id]()}</span>
          <span class="key-area">{a.area === 'editor' ? m.key_area_editor() : m.key_area_app()}</span>
          {#if capturing === a.id}
            <button class="btn key-capture" type="button" use:listen={a.id} onblur={() => (capturing = null)}>{m.key_press()}</button>
          {:else}
            <button class="btn key-combo" type="button" aria-label="{ACTION_NAME[a.id]()}: {show(bound[a.id])}. {m.key_change()}" onclick={() => begin(a.id)}>
              {#if bound[a.id]}<kbd>{show(bound[a.id])}</kbd>{:else}<span class="unbound">{m.key_unbound()}</span>{/if}
            </button>
          {/if}
          <span class="key-tools">
            {#if changed(a.id)}<button class="btn ghost compact" type="button" onclick={() => reset(a.id)}>{m.key_default()}</button>{/if}
            {#if bound[a.id]}<button class="btn ghost compact" type="button" onclick={() => unbind(a.id)}>{m.key_unbind()}</button>{/if}
          </span>
          {#if refusal?.id === a.id}<p class="key-refusal" role="alert">{refusal.text}</p>{/if}
        </li>
      {/each}
      {#each FIXED.filter((f) => f.key !== 'redo_alt') as f (f.key)}
        <li class="key-row fixed">
          <span class="key-name">{(FIXED_NAME[f.key] ?? (() => f.key))()}</span>
          <span class="key-area"><Lock size={11} strokeWidth={1.8} aria-hidden="true" /> {m.key_fixed()}</span>
          <kbd>{show(f.combo)}</kbd>
          <span></span>
        </li>
      {/each}
    </ul>
  </section>
  {/if}

  {#if error}<p class="error" role="alert">{error}</p>{/if}
</div>
