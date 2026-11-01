<script lang="ts">
  import { tick } from 'svelte';
  import type { ChoiceId, Driver, DriverKind } from '../lib/drivers';
  import { ICON } from '../lib/icons';
  import { engineRows, engineSections, stepEngine, type EngineGroup, type EngineRow } from '../lib/engines';
  import { m } from '../paraglide/messages';
  import DriverMark from './DriverMark.svelte';

  let {
    value,
    usable,
    onpick,
    onenter,
    search = $bindable(),
  }: { value: ChoiceId; usable: (id: ChoiceId) => boolean; onpick: (id: ChoiceId) => void; onenter: () => void; search?: HTMLInputElement } = $props();

  const GROUP: Record<EngineGroup, () => string> = {
    relational: m.eng_group_relational,
    document: m.eng_group_document,
    keyvalue: m.eng_group_keyvalue,
    widecolumn: m.eng_group_widecolumn,
    search: m.eng_group_search,
    analytical: m.eng_group_analytical,
    enterprise: m.eng_group_enterprise,
    odbc: m.eng_group_odbc,
  };
  const KIND: Record<DriverKind, () => string> = {
    relational: m.kind_relational,
    document: m.kind_document,
    keyvalue: m.kind_keyvalue,
    analytical: m.kind_analytical,
    widecolumn: m.kind_widecolumn,
    search: m.kind_search,
  };
  const where = (d: Driver) => (d.where === 'file' ? m.where_file() : d.where === 'cloud' ? m.where_cloud() : m.where_server());
  const words = (r: EngineRow) => `${GROUP[r.group]()} ${KIND[r.driver.kind]()} ${where(r.driver)}`;

  const rows = engineRows();
  let query = $state('');
  const sections = $derived(engineSections(rows, query, words));
  const visible = $derived(sections.flatMap((s) => s.rows.map((r) => r.id)));

  function go(id: ChoiceId | null) {
    if (id && id !== value && usable(id)) onpick(id);
  }

  function filter(text: string) {
    query = text;
    if (!visible.includes(value)) go(visible.find(usable) ?? null);
  }

  function keys(e: KeyboardEvent) {
    const k = e.key;
    if (k === 'ArrowDown' || k === 'ArrowUp' || k === 'PageDown' || k === 'PageUp') {
      e.preventDefault();
      go(stepEngine(visible, value, k === 'ArrowDown' || k === 'PageDown' ? 1 : -1, usable, k.startsWith('Page') ? 8 : 1));
    } else if (k === 'Enter') {
      e.preventDefault();
      onenter();
    } else if (k === 'Escape' && query) {
      e.preventDefault();
      e.stopPropagation();
      filter('');
    }
  }

  $effect(() => {
    const at = `eng-${value}`;
    void sections;
    void tick().then(() => document.getElementById(at)?.scrollIntoView({ block: 'nearest' }));
  });
</script>

<div class="eng-rail">
  <label class="field eng-search">
    <ICON.searchObjects size={13} strokeWidth={1.8} aria-hidden="true" />
    <input
      bind:this={search}
      type="search"
      role="combobox"
      aria-expanded="true"
      aria-controls="eng-list"
      aria-autocomplete="list"
      aria-activedescendant={visible.includes(value) ? `eng-${value}` : undefined}
      aria-label={m.eng_search_label()}
      placeholder={m.eng_search()}
      value={query}
      autocomplete="off"
      spellcheck="false"
      oninput={(e) => filter(e.currentTarget.value)}
      onkeydown={keys}
    />
  </label>
  <div class="eng-list" id="eng-list" role="listbox" aria-label={m.form_driver()} tabindex="-1" onmousedown={(e) => e.preventDefault()}>
    {#each sections as s (s.group)}
      <div class="ndsel-group eng-group" role="group" aria-label={GROUP[s.group]()}>
        <div class="ndsel-group-name" aria-hidden="true">{GROUP[s.group]()}</div>
        {#each s.rows as r (r.id)}
          {@const locked = !usable(r.id)}
          <div
            id="eng-{r.id}"
            class="ndsel-opt eng-opt"
            class:indent={r.indent}
            role="option"
            tabindex="-1"
            aria-selected={r.id === value}
            aria-disabled={locked || undefined}
            data-tip={locked ? m.eng_locked() : undefined}
            onclick={() => {
              go(r.id);
              search?.focus();
            }}
            onkeydown={() => {}}
          >
            <span class="ndsel-mark" aria-hidden="true"></span>
            <span class="ndsel-text eng-text">
              <DriverMark driver={r.driver} size={16} />
              <span class="eng-name">{r.driver.name}</span>
              {#if r.driver.where !== 'server'}<span class="eng-where">{where(r.driver)}</span>{/if}
            </span>
          </div>
        {/each}
      </div>
    {:else}
      <p class="eng-none">{m.eng_none({ query: query.trim() })}</p>
    {/each}
  </div>
  <p class="eng-keys" aria-hidden="true">
    <span class="eng-key"><kbd>↑</kbd><kbd>↓</kbd> {m.eng_keys_pick()}</span>
    <span class="eng-key"><kbd>{m.eng_keys_enter()}</kbd> {m.eng_keys_fields()}</span>
  </p>
</div>
